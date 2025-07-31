use std::env;
use secrecy::Secret;
use serde::Deserialize;

use crate::llm::{services::{DeepseekAuth, GenericLLMService, GigaChatAuth}, LLMService};

#[derive(Debug, Clone, Deserialize)]
pub enum Model {
    GigaChat(Option<ModelData>),
    DeepSeek(Option<ModelData>)
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelData {
    pub token: Secret<String>,
    pub scope: Option<String>
}

macro_rules! create_service {
    ($c:ident, $t:ty) => {
        match GenericLLMService::<$t>::create($c).await {
            Ok(service) => Some(Box::new(service)),
            Err(e) => {
                println!("Ошибка при создании сервиса {:?}", e);
                None
            }
        }
    };
}

impl Model {
    pub fn to_string(&self) -> String {
        match self {
            Model::GigaChat(_) => "gigachat".into(),
            Model::DeepSeek(_) => "deepseek".into(),
        }
    }

    pub async fn get_service(&self) -> Option<Box<dyn LLMService>> {
        match self {
            Model::GigaChat(Some(data)) => create_service!(data, GigaChatAuth),
            Model::DeepSeek(Some(data)) => create_service!(data, DeepseekAuth),
            _ => None
        }
    }

    pub fn set_data(&mut self, data: ModelData) {
        match self {
            Model::GigaChat(_) => *self = Model::GigaChat(Some(data)),
            Model::DeepSeek(_) => *self = Model::DeepSeek(Some(data)),
        }
    }
}

pub fn load() -> Vec<Model> {
    let mut models = vec![
        Model::GigaChat(None), Model::DeepSeek(None)
    ];
    dotenvy::dotenv().ok();
    
    for model in &mut models {
        let model_name = model.to_string().to_uppercase();

        if let Ok(token) = env::var(
            format!("TOKEN_{}", model_name)
        ) {
            let data = ModelData {
                token: Secret::new(token),
                scope: match env::var(format!("SCOPE_{}", model_name)) {
                    Ok(scope) => Some(scope),
                    _ => None
                },
            };
            model.set_data(data);
        }
    }

    models
}
