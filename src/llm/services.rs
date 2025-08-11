use std::str::FromStr;
use std::{env, fs};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, Method, RequestBuilder};
use ringbuffer::{AllocRingBuffer, RingBuffer};
use secrecy::{ExposeSecret, Secret};
use serde_json::json;
use tokio::sync::RwLock;
use tool_registry::ToolRegistry;
use crate::config::ModelData;
use crate::llm::provider::{ServiceChatRequest, ServiceChatResponse, ServiceEmbeddingRequest, ServiceEmbeddingResponse};
use crate::llm::{ChatMessage, EmbeddedRequest, EmbeddedResponse, Tool, ToolChoice, HISTORY_SIZE};
use crate::llm::{auth::TokenInterceptor, ChatRequest, ChatResponse, LLMService, RETRIES};

use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};

#[derive(Clone)]
pub struct GenericLLMService<A> {
    auth: A,
    client: ClientWithMiddleware,
    tools_registry: Arc<RwLock<ToolRegistry>>,
    base_url: String
}

impl<A> GenericLLMService<A> {
    pub async fn new(auth: A, base_url: &str) -> anyhow::Result<Self> {
        let retry_policy = ExponentialBackoff::builder()
            .build_with_max_retries(RETRIES);
  
  // connetion timeout в ылетает, не пытает делать retry
        // let client = Client::builder()
        //     .connect_timeout(Duration::from_secs(TIMEOUT))
        //     .timeout(Duration::from_secs(TIMEOUT))
        //     .build()?;

        let client = ClientBuilder::new(Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        
        let instance = Self {
            auth,
            client,
            tools_registry: Arc::new(RwLock::new(ToolRegistry::new())),
            base_url: base_url.to_string()
        };

        instance.start_tool_watcher().await;
        Ok(instance)
    }

    pub async fn start_tool_watcher(&self) {
        let tool_registry = self.tools_registry.clone();
        let tools_dir_name = match env::var("TOOLS_PATH") {
            Ok(path) => path,
            Err(_) => {
                println!("TOOLS_PATH переменная окружения не установлена. Используется значение по умолчанию.");
                "tools".to_string()
            }
        };
        let tools_dir = PathBuf::from(tools_dir_name.clone());

        match fs::create_dir(tools_dir.clone()) {
            Ok(_) => println!("Директория {} успешно создана.", tools_dir_name),
            Err(e) => println!("Директория {tools_dir_name} не может быть создана: {}", e)
        };
    
        if let Err(e) = tool_registry.write().await.load_from_dir(&tools_dir) {
            eprintln!("Failed to load tools: {}", e);
        }
    
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                
                if let Err(e) = tool_registry.write().await.load_from_dir(&tools_dir) {
                    eprintln!("Failed to reload tools: {}", e);
                }
            }
        });
    }
}

#[async_trait]
impl<A: AuthProvider + Sync + Send + Clone +'static> LLMService for GenericLLMService<A> {
    async fn chat(&self, request: ServiceChatRequest) -> Result<ServiceChatResponse, Box<dyn std::error::Error>> {
        let mut history = AllocRingBuffer::with_capacity(HISTORY_SIZE);
        let mut response = self._send(
            request.messages, &mut history,
            request.model.clone(), request.temperature
        ).await?;

        println!("HERE {:?}", response);

        if let Some(message) = &response.choices[0].message {
            if let Some(tool_calls) = &message.tool_calls {
                let mut tool_buffer = Vec::new();
                for tool_call in tool_calls {
                    let tool_registry = self.tools_registry.read().await;
                    let name = &tool_call.function.name;

                    if let Some(tool) = tool_registry.get_tool(name) {
                        let arguments = serde_json::Value::from_str(
                            &tool_call.function.arguments.clone()
                        )?;
                        let tool_responce = match tool.execute(arguments).await {
                            Ok(result)=> result,
                            Err(e) => {
                                json!({
                                    "error": e.to_string()
                                })
                            }
                        };

                        println!("HERE {:?}", tool_responce);

                        tool_buffer.push(ChatMessage {
                            role: "tool".into(),
                            content: Some(serde_json::to_string(&tool_responce)?),
                            tool_calls: None,
                            tool_call_id: Some(tool_call.id.clone()),
                            name: Some(name.clone()),
                        });
                    } else {
                        println!(
                            "Инструмент не найден: {}",
                            tool_call.function.name
                        );
                    }
                }
 
                response = self._send(
                    tool_buffer,
                    &mut history,
                    request.model.clone(),
                    request.temperature
                ).await?;
            }
        }
        history.clear();

        let message = response.choices
            .first()
            .and_then(|choice| choice.message.as_ref())
            .ok_or_else(|| anyhow::anyhow!("No valid message in response"))?;

        Ok(ServiceChatResponse { content: message.content.clone() })
    }

    async fn embedded(&self, request: ServiceEmbeddingRequest) -> Result<ServiceEmbeddingResponse, Box<dyn std::error::Error>> {
        let body = EmbeddedRequest {
            model: request.model,
            input: vec![request.input],
        };

        let body = serde_json::to_vec(&body)?;

        let request = self.auth.with_auth(
            Client::new()
                .request(Method::POST, format!("{}/embeddings", self.base_url))
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
                .body(body)
        ).build()?;

        let response = self.client.execute(request).await?;
        let response = response.text().await?;
        let response = serde_json::from_str::<EmbeddedResponse>(&response)?;

        Ok(ServiceEmbeddingResponse { content: response.data[0].embedding.clone() })
    }
}

impl<A: AuthProvider + Sync + Send + Clone +'static>  GenericLLMService<A> {
    async fn _send(
        &self,
        messages: Vec<ChatMessage>,
        history: &mut AllocRingBuffer<ChatMessage>,
        model: String,
        temperature: f32
    ) -> Result<ChatResponse, Box<dyn std::error::Error>> {
        let tools = self.tools_registry.read().await;
        let tools = match serde_json::from_value::<Vec<Tool>>(
            serde_json::json!(tools.tools_specs())
        )? {
            vec if !vec.is_empty() => Some(vec),
            _ => None,
        };

        history.extend(messages);

        let body = ChatRequest {
            model: model,
            messages: history.to_vec(),
            temperature: Some(temperature),
            tools: tools,
            tool_choice: Some(ToolChoice::Auto),
        };

        let body = serde_json::to_vec(&body)?;

        
        let request = self.auth.with_auth(
        Client::new()
            .request(Method::POST, format!("{}/chat/completions", self.base_url))
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .body(body)
        ).build()?;
        
        let response = self.client.execute(request).await?;
        let response = response.text().await?;

        println!("HERE RESPONCE {:?}", response);
        
        let response = serde_json::from_str::<ChatResponse>(&response)?;
        if let Some(message) = &response.choices[0].message {
            history.enqueue(message.clone());
        }

        Ok(response)
    }
}


pub trait AuthProvider {
    fn with_auth(&self, req: RequestBuilder) -> RequestBuilder;
}

// Реализация для GigaChat

#[derive(Clone)]
pub struct GigaChatAuth {
    token_interceptor: TokenInterceptor,
}

impl GigaChatAuth {
    pub async fn new(api_key: Secret<String>, scope: Option<String>) -> anyhow::Result<Self> {
        let token_interceptor = TokenInterceptor::new(
            api_key, match scope {
                Some(scope) => scope,
                None => "GIGACHAT_API_PERS".into()
            }, "https://ngw.devices.sberbank.ru:9443/api/v2/oauth".into()
        ).await?;
        Ok(Self { token_interceptor })
    }
}

impl AuthProvider for GigaChatAuth {
    fn with_auth(&self, req: RequestBuilder) -> RequestBuilder {
        req.header("Authorization", format!("Bearer {}", self.token_interceptor.get_token()))
    }
}

// Реализация для Deepseek
#[derive(Clone)]
pub struct DeepseekAuth {
    api_key: Secret<String>,
}

impl AuthProvider for DeepseekAuth {
    
    fn with_auth(&self, req: RequestBuilder) -> RequestBuilder {
        req.header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))

    }
}

impl GenericLLMService<GigaChatAuth> {
    pub async fn create(config: &ModelData) -> anyhow::Result<Self> {
        let auth = GigaChatAuth::new(
            config.token.clone(),
            config.scope.clone()
        ).await?;
        
        Self::new(auth, "https://gigachat.devices.sberbank.ru/api/v1").await
    }
}

impl GenericLLMService<DeepseekAuth> {
    pub async fn create(config: &ModelData) -> anyhow::Result<Self> {
        let auth = DeepseekAuth {
            api_key: config.token.clone()
        };
        
        Self::new(auth, "https://api.deepseek.com").await
    }
}