use async_trait::async_trait;
use dotenv::dotenv;
use std::env;

use llmgraph::generate::generate::generate_full_response;
use llmgraph::{Agent as LlmAgentTrait, Message};

pub struct FirstAgent;
pub struct SecondAgent;

#[async_trait]
impl LlmAgentTrait for FirstAgent {
    async fn run(
        &mut self,
        _input: &str,
        tool_registry: &(dyn llmgraph::ToolRegistryTrait + Send + Sync),
    ) -> (String, Option<i32>) {
        dotenv().ok();
        let api_key = env::var("API_KEY").unwrap_or_default();
        let base_url = "https://openrouter.ai/api/v1/chat/completions".to_string();
        let model = "z-ai/glm-4.5".to_string();
        let temperature = 0.1;

        let mut messages = vec![
            Message {
                role: "system".into(),
                content: Some("Security check passed".into()),
                tool_calls: None,
            },
            Message {
                role: "user".into(),
                content: Some("List files and get secret".into()),
                tool_calls: None,
            },
        ];

        let tools = tool_registry.get_tools();
        for _ in 0..3 {
            let resp = generate_full_response(
                base_url.clone(),
                api_key.clone(),
                model.clone(),
                temperature,
                messages.clone(),
                Some(tools.clone()),
            )
            .await;

            let llm = match resp {
                Ok(r) => r,
                Err(e) => return (format!("Error: {}", e), None),
            };
            let choice = &llm.choices[0];
            let msg = &choice.message;

            messages.push(Message {
                role: "assistant".into(),
                content: msg.content.clone(),
                tool_calls: msg.tool_calls.clone(),
            });

            if let Some(tool_calls) = &msg.tool_calls {
                for tc in tool_calls {
                    let result = tool_registry
                        .execute_tool(&tc.function.name, &tc.function.arguments);
                    let content = match result {
                        Ok(v) => serde_json::to_string(&v).unwrap(),
                        Err(e) => format!("Error: {}", e),
                    };
                    messages.push(Message {
                        role: "tool".into(),
                        content: Some(content),
                        tool_calls: None,
                    });
                }
                continue;
            }

            if let Some(content) = &msg.content {
                return (
                    format!(
                        "Write a summary of changes needed reading this json: {}",
                        content
                    ),
                    Some(1),
                );
            }
        }
        ("FirstAgent: max iterations reached".into(), Some(1))
    }

    fn get_name(&self) -> &str {
        "FirstAgent"
    }
}

#[async_trait]
impl LlmAgentTrait for SecondAgent {
    async fn run(
        &mut self,
        input: &str,
        _tools: &(dyn llmgraph::ToolRegistryTrait + Send + Sync),
    ) -> (String, Option<i32>) {
        dotenv().ok();
        let api_key = env::var("API_KEY").unwrap_or_default();
        let base_url = "https://openrouter.ai/api/v1/chat/completions".to_string();
        let model = "z-ai/glm-4.5".to_string();
        let temperature = 0.1;

        let messages = vec![Message {
            role: "system".into(),
            content: Some(input.to_string()),
            tool_calls: None,
        }];

        let llm = generate_full_response(
            base_url,
            api_key,
            model,
            temperature,
            messages,
            None,
        )
        .await;

        (
            format!(
                "Second processed: {:?}",
                llm.ok()
                    .and_then(|r| r.choices.first().and_then(|c| c.message.content.clone()))
                    .unwrap_or_else(|| "no content".into())
            ),
            None,
        )
    }

    fn get_name(&self) -> &str {
        "SecondAgent"
    }
}