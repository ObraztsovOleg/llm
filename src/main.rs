use axum::{
    routing::post,
    Router, Json,
    extract::State,
    http::StatusCode,
};
use std::sync::Arc;
use crate::llm::{provider::{LlmProvider, ServiceChatRequest, ServiceChatResponse, ServiceEmbeddingRequest, ServiceEmbeddingResponse}};
mod llm;
mod config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = Arc::new(LlmProvider::new().await?);
    
    let app = Router::new()
        .route("/chat", post(handle_chat))
        .route("/embedding", post(handle_embedding))
        .with_state(service);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn handle_chat(
    State(service): State<Arc<LlmProvider>>,
    Json(request): Json<ServiceChatRequest>,
) -> Result<Json<ServiceChatResponse>, (StatusCode, String)> {
    service
        .chat(request).await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

async fn handle_embedding(
    State(service): State<Arc<LlmProvider>>,
    Json(request): Json<ServiceEmbeddingRequest>,
) -> Result<Json<ServiceEmbeddingResponse>, (StatusCode, String)> {
    service
        .embedding(request).await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}
