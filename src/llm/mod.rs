pub mod auth;
pub mod provider;
pub mod services;

use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use serde_json::Value;

use crate::llm::provider::{ServiceChatRequest, ServiceChatResponse, ServiceEmbeddingRequest, ServiceEmbeddingResponse};

const RETRIES: u32 = 100;
const HISTORY_SIZE: usize = 100;
const TIMEOUT: u64 = 100;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: FunctionCall,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Auto,
    None,
    SpecificTool {
        #[serde(rename = "type")]
        type_: String,
        function: FunctionToolChoice,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionToolChoice {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub type_: String,
    pub function: FunctionSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionSpec {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Value, // JSON schema
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: Option<String>,
    pub object: Option<String>,
    pub created: Option<i64>,
    pub model: Option<String>,
    pub choices: Vec<Alternative>,
    pub usage: Option<Usage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alternative {
    pub index: i32,
    pub message: Option<ChatMessage>,
    pub finish_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<DeltaMessage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeltaMessage {
    pub role: Option<String>,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmbeddedResponse {
    pub object: String,
    pub model: String,
    pub data: Vec<EmbeddedData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmbeddedData {
    pub object: String,
    pub index: u32,
    pub embedding: Vec<f32>,
    pub usage: EmbeddedUsage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmbeddedUsage {
    pub prompt_tokens: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmbeddedRequest {
    pub model: String,
    pub input: Vec<String>,
}

#[async_trait]
pub trait LLMService: Send + Sync {
    async fn chat(&self, request: ServiceChatRequest) -> Result<ServiceChatResponse, Box<dyn std::error::Error>>;
    async fn embedded(&self, request: ServiceEmbeddingRequest) -> Result<ServiceEmbeddingResponse, Box<dyn std::error::Error>>;
}
