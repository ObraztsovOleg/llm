use async_trait::async_trait;
use serde_json::Value;
use std::error::Error;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, arguments: Value) -> Result<Value, Box<dyn Error>>;
}


pub type CreateToolFn = unsafe extern fn() -> *mut dyn Tool;