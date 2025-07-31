use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use crate::{
    config::load, llm::{ChatMessage, LLMService}
};

#[derive(Clone, Debug, Deserialize)]
pub struct ServiceChatRequest {
    pub provider: String,
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default = "default_temperature")]
    pub temperature: f32
}
fn default_temperature() -> f32 { 0.1 }

#[derive(Clone, Debug, Deserialize)]
pub struct ServiceEmbeddingRequest {
    pub provider: String,
    pub model: String,
    pub input: String
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceChatResponse {
    pub content: Option<String>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceEmbeddingResponse {
    pub content: Vec<f32>
}

pub struct LlmProvider {
    providers: HashMap<String, Box<dyn LLMService>>
}

impl LlmProvider {
    pub async fn new() -> anyhow::Result<Self> {
        let mut providers = HashMap::<String, Box<dyn LLMService>>::new();
        
        let llms = load();
        for llm in llms {
            match llm.get_service().await {
                Some(service) => {
                    providers.insert(
                        llm.to_string().to_uppercase(),
                        service
                    ); 
                },
                None => println!("Невозможно получить сервис {:?}", llm.to_string()),
            } 
        }
        
        Ok(Self { providers })
    }
    
    pub async fn chat(
        &self,
        request: ServiceChatRequest,
    ) -> Result<ServiceChatResponse, Box<dyn std::error::Error>> {
        self.providers
            .get(&request.provider.to_uppercase())
            .ok_or_else(|| anyhow::anyhow!(
                "Модель - {} - не поддерживается", request.provider.to_uppercase()
            ))?
            .chat(request).await
    }

    pub async fn embedding(
        &self,
        request: ServiceEmbeddingRequest,
    ) -> Result<ServiceEmbeddingResponse, Box<dyn std::error::Error>> {
        self.providers
            .get(&request.provider.to_uppercase())
            .ok_or_else(|| anyhow::anyhow!(
                "Модель - {} - не поддерживается", request.provider.to_uppercase()
            ))?
            .embedded(request).await
    }
}
