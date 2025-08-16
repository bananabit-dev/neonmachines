use std::collections::HashMap;
use std::env;

use async_trait::async_trait;
use dotenv::dotenv;
use llmgraph::Agent;
use llmgraph::Graph;
use llmgraph::Message;
use llmgraph::Tool;
use llmgraph::generate::generate::generate_full_response;
use llmgraph::models::tools::Function;
use llmgraph::models::tools::Parameters;
use llmgraph::models::tools::Property;

struct FirstAgent;
struct SecondAgent;

#[async_trait]
impl Agent for FirstAgent {
    async fn run(
        &mut self,
        input: &str,
        tool_registry: &(dyn llmgraph::ToolRegistryTrait + Send + Sync),
    ) -> (String, Option<i32>) {
        dotenv::dotenv().ok();
        let api_key = env::var("API_KEY").unwrap();
        let base_url = "https://openrouter.ai/api/v1/chat/completions".to_string();
        let model = "z-ai/glm-4.5".to_string();
        let temperature = 0.1;

        let mut messages = vec![
            Message {
                role: "system".to_string(),
                content: Some("You are a helpful assistant. Use the available tools if needed.".to_string()),
                tool_calls: None,
            },
            Message {
                role: "user".to_string(),
                content: Some(input.to_string()),
                tool_calls: None,
            },
        ];

        let tools = tool_registry.get_tools();
        let max_iterations = 5;

        for _ in 0..max_iterations {
            let response = generate_full_response(
                base_url.clone(),
                api_key.clone(),
                model.clone(),
                temperature,
                messages.clone(),
                Some(tools.clone()),
            ).await;

            let llm_response = match response {
                Ok(r) => r,
                Err(e) => return (format!("Error: {}", e), None),
            };

            let choice = &llm_response.choices[0];
            let assistant_message = &choice.message;

            // Add assistant message
            messages.push(Message {
                role: "assistant".to_string(),
                content: assistant_message.content.clone(),
                tool_calls: assistant_message.tool_calls.clone(),
            });

            if let Some(tool_calls) = &assistant_message.tool_calls {
                // Execute each tool
                for tool_call in tool_calls {
                    let result = tool_registry.execute_tool(
                        &tool_call.function.name,
                        &tool_call.function.arguments,
                    );

                    let result_content = match result {
                        Ok(val) => serde_json::to_string(&val).unwrap(),
                        Err(e) => format!("Error: {}", e),
                    };

                    // Add tool result back into conversation
                    messages.push(Message {
                        role: "tool".to_string(),
                        content: Some(result_content),
                        tool_calls: None,
                    });
                }
                continue; // loop again, let LLM process tool results
            }

            // If we got plain text, return it
            if let Some(content) = &assistant_message.content {
                return (format!("First processed: {}", content), Some(1));
            }
        }

        ("FirstAgent: max iterations reached".to_string(), Some(1))
    }

    fn get_name(&self) -> &str {
        "FirstAgent"
    }
}

#[async_trait]
impl Agent for SecondAgent {
    async fn run(
        &mut self,
        input: &str,
        _: &(dyn llmgraph::ToolRegistryTrait + Send + Sync),
    ) -> (String, Option<i32>) {
        dotenv().ok();

        let api_key = env::var("API_KEY").unwrap();
        let base_url = "https://openrouter.ai/api/v1/chat/completions".to_string();
        let model = "z-ai/glm-4.5".to_string();
        let temperature = 0.1;
        let messages: Vec<Message> = vec![Message {
            role: "system".to_string(),
            content: Some(input.to_string()),
            tool_calls: None,
        }];
        let llm_response =
            generate_full_response(base_url, api_key, model, temperature, messages, None).await;
        dbg!(&llm_response);
        (
            format!(
                "Second processed: {:?}",
                llm_response.unwrap().choices[0].message.content
            ),
            None,
        ) // stop here
    }
    fn get_name(&self) -> &str {
        "SecondAgent"
    }
}

#[tokio::main]
async fn main() {
    let mut graph = Graph::new();

    // Define a weather tool
    fn get_secret_tool() -> Tool {
        Tool {
            tool_type: "function".to_string(),
            function: Function {
                name: "get_secret".to_string(),
                description: "Get the current secret".to_string(),
                parameters: Parameters {
                    param_type: "object".to_string(),
                    properties: {
                        let props = HashMap::new();
                        props
                    },
                    required: vec![],
                },
            },
        }
    }
    let secret_tool = get_secret_tool();

    graph.register_tool(secret_tool, |args| {
        Ok(serde_json::json!({
            "secret": "rust is blazingly fast! :)"
        }))
    });

    graph.add_node(0, Box::new(FirstAgent));
    graph.add_node(1, Box::new(SecondAgent));
    graph.add_edge(0, 1).unwrap();
    graph.print();

    let result = graph.run(0, "What Tools do you have and can you get my secret can you send that secret using your tools in your words?").await;
    println!("Result: {}", result);
}
