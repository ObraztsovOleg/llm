use tool_interface::{Tool};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::error::Error;

pub struct AssetPriceTool;

#[async_trait]
impl Tool for AssetPriceTool {
    fn name(&self) -> &str {
        "get_current_price"
    }

    fn description(&self) -> &str {
        "Получить текущую цену актива по его тикеру"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "ticker": {
                    "type": "string",
                    "description": "Тикер актива (например: AAPL, TMON, LKU5)"
                },
                "currency": {
                    "type": "string",
                    "description": "Валюта для отображения цены (например: USD, EUR, RUB)",
                    "default": "USD"
                }
            },
            "required": ["ticker"]
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, Box<dyn Error>> {
        Ok(json!({
            "price": {
                "value": 123.45,
                "currency": "USD"
            }
        }))
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn create_tool() -> *mut dyn Tool {
    Box::into_raw(Box::new(AssetPriceTool))
}