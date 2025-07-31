use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use chrono::{DateTime, Utc};
use chrono::serde::ts_milliseconds;

use reqwest::Client;
use reqwest_middleware::ClientBuilder;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use secrecy::{ExposeSecret, Secret};
use tonic::service::Interceptor;
use uuid::Uuid;

use crate::llm::{RETRIES, TIMEOUT};


#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct LLMAuthResponse {
    access_token: String,
    #[serde(with = "ts_milliseconds")]
    expires_at: DateTime<Utc>
}

async fn auth(token: &str, scope: String, auth_url: String) -> anyhow::Result<LLMAuthResponse> {
    let retry_policy = ExponentialBackoff::builder()
        .build_with_max_retries(RETRIES);

    let client = Client::builder()
        .use_native_tls()
        .connect_timeout(Duration::from_secs(TIMEOUT))
        .timeout(Duration::from_secs(TIMEOUT))
        .build()?;

    let client = ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();

    let mut params = HashMap::new();
    params.insert("scope", scope);
    let responce = client
        .post(auth_url)
        .header("Accept", "application/json")
        .header("RqUID", Uuid::new_v4().to_string())
        // .header("RqUID", "1994ff12-ad06-47d6-84ac-45e2d8972fd9")
        .header("Authorization", format!("Basic {token}"))
        .form(&params)
        .send()
        .await?;

    
    if !responce.status().is_success() {
        return Err(anyhow::anyhow!(
            "Auth failed: {} {}",
            responce.status(), responce.text().await?
        ));
    }

    let responce = serde_json::from_str::<LLMAuthResponse>(&responce.text().await?)?;

    Ok(responce)
}

#[derive(Clone, Debug)]
pub struct TokenInterceptor {
    token: Arc<RwLock<String>>,
}

impl TokenInterceptor {
    pub async fn new(auth_token: Secret<String>, scope: String, auth_url: String) -> anyhow::Result<Self> {
        let LLMAuthResponse { access_token, expires_at } = auth(auth_token.expose_secret(), scope.clone(), auth_url.clone()).await?;
        let token = Arc::new(RwLock::new(access_token));
        let updatable = Arc::downgrade(&token);

        tokio::spawn(async move {
            tokio::time::sleep((expires_at - Utc::now()).to_std().unwrap_or(Duration::from_secs(60))).await;

            while let Some(updatable) = updatable.upgrade() {
                let LLMAuthResponse { access_token, expires_at } = match auth(auth_token.expose_secret(), scope.clone(), auth_url.clone()).await {
                    Ok(t) => t,
                    Err(_err) => {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue
                    }
                };

                *updatable.write().unwrap() = access_token;
                let sleep_duration = expires_at - Utc::now();
                tokio::time::sleep(sleep_duration.to_std().unwrap_or(Duration::from_secs(5))).await;
            }
        });

        Ok(Self { token })
    }

    pub fn get_token(&self) -> String {
        self.token.read().unwrap().clone()
    }
}

impl Interceptor for TokenInterceptor {
    fn call(&mut self, mut req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        req.metadata_mut().append(
            "authorization",
            format!("Bearer {}", self.token.read().unwrap()).parse().unwrap(),
        );

        Ok(req)
    }
}
