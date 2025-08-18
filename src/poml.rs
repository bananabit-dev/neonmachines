use crate::runner::AppEvent;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;
use anyhow::Result;

pub struct PomlExecutor {
    tx: UnboundedSender<AppEvent>,
}

impl PomlExecutor {
    pub fn new(tx: UnboundedSender<AppEvent>) -> Self {
        Self { tx }
    }

    pub async fn execute_poml_file(&self, file_path: &PathBuf, working_dir: Option<PathBuf>) -> Result<()> {
        // Check if file exists
        if !file_path.exists() {
            let _ = self.tx.send(AppEvent::Log(format!("Error: POML file not found: {}", file_path.display())));
            return Ok(());
        }

        // Read the POML file
        let poml_content = match tokio::fs::read_to_string(file_path).await {
            Ok(content) => content,
            Err(e) => {
                let _ = self.tx.send(AppEvent::Log(format!("Error reading POML file: {}", e)));
                return Ok(());
            }
        };

        // Extract variables from POML (simplified parsing)
        let variables = self.extract_variables_from_poml(&poml_content).await?;
        
        // Send execution start event
        let _ = self.tx.send(AppEvent::Log(format!("Executing POML file: {}", file_path.display())));
        let default_dir = PathBuf::from(".");
        let _ = self.tx.send(AppEvent::Log(format!("Working directory: {:?}", working_dir.as_deref().unwrap_or(&default_dir))));

        // Here we would integrate with the actual POML execution
        // For now, we'll just log the extracted variables
        for (name, value) in &variables {
            let _ = self.tx.send(AppEvent::Log(format!("Variable: {} = {}", name, value)));
        }

        let _ = self.tx.send(AppEvent::Log("POML execution completed".to_string()));
        Ok(())
    }

    async fn extract_variables_from_poml(&self, poml_content: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut variables = std::collections::HashMap::new();
        
        // Simple regex-based extraction of variables
        let re = regex::Regex::new(r#"<let\s+name="([^"]+)"[^>]*value="([^"]*)"[^>]*>"#)?;
        if let Some(captures) = re.captures(poml_content) {
            if let Some(name) = captures.get(1) {
                if let Some(value) = captures.get(2) {
                    variables.insert(name.as_str().to_string(), value.as_str().to_string());
                }
            }
        }

        // Extract prompt variables
        let prompt_re = regex::Regex::new(r"\{\{\s*prompt\s*\}\}")?;
        if prompt_re.is_match(poml_content) {
            variables.insert("prompt".to_string(), "User input".to_string());
        }

        // Extract input variables
        let input_re = regex::Regex::new(r"\{\{\s*input\s*\}\}")?;
        if input_re.is_match(poml_content) {
            variables.insert("input".to_string(), "User input".to_string());
        }

        Ok(variables)
    }
}

pub async fn handle_poml_execution(
    file_path: &PathBuf,
    working_dir: Option<PathBuf>,
    tx: UnboundedSender<AppEvent>,
) -> Result<()> {
    let executor = PomlExecutor::new(tx);
    executor.execute_poml_file(file_path, working_dir).await
}
