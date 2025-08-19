use crate::runner::AppEvent;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;
use anyhow::Result;
use tokio::process::Command;
use std::process::Stdio;

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

        // Send execution start event
        let _ = self.tx.send(AppEvent::Log(format!("Executing POML file via external CLI: {}", file_path.display())));
        
        let current_dir = if let Some(ref dir) = working_dir {
            dir.clone()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        
        let _ = self.tx.send(AppEvent::Log(format!("Working directory: {:?}", current_dir)));

        // Build the external poml-cli command
        let mut command = Command::new("python");
        command.arg("-m").arg("poml");
        command.arg("-f").arg(file_path.display().to_string());
        
        // Set working directory if specified
        command.current_dir(&current_dir);

        // Add environment variables that might be needed
        command.env("POML_WORKING_DIR", current_dir.display().to_string());

        // Set up stdout and stderr capture
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        // Log the command being executed
        let command_str = format!("python -m poml -f {}", file_path.display());
        let _ = self.tx.send(AppEvent::Log(format!("Executing: {}", command_str)));

        // Spawn the command
        let child = command
            .spawn()
            .map_err(|e| {
                let error_msg = format!("Failed to start poml-cli: {}", e);
                let _ = self.tx.send(AppEvent::Log(format!("Error: {}", error_msg)));
                anyhow::anyhow!(error_msg)
            })?;

        // Wait for the command to complete
        let output = child
            .wait_with_output()
            .await
            .map_err(|e| {
                let error_msg = format!("Failed to wait for poml-cli execution: {}", e);
                let _ = self.tx.send(AppEvent::Log(format!("Error: {}", error_msg)));
                anyhow::anyhow!(error_msg)
            })?;

        // Process the output
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let _ = self.tx.send(AppEvent::Log(format!("POML execution successful")));
            let _ = self.tx.send(AppEvent::Log(format!("Output:\n{}", stdout)));
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error_msg = format!("POML execution failed: {}", stderr);
            let _ = self.tx.send(AppEvent::Log(format!("Error: {}", error_msg)));
            return Err(anyhow::anyhow!(error_msg));
        }

        // Check if poml-cli is available
        self.check_poml_cli_availability().await?;

        Ok(())
    }

    async fn check_poml_cli_availability(&self) -> Result<()> {
        // Check if python is available
        let python_check = Command::new("python")
            .arg("--version")
            .output()
            .await;

        match python_check {
            Ok(output) if !output.status.success() => {
                let _ = self.tx.send(AppEvent::Log("Error: Python is not available".to_string()));
                return Err(anyhow::anyhow!("Python is not available"));
            }
            Err(e) => {
                let _ = self.tx.send(AppEvent::Log(format!("Error: Failed to check Python: {}", e)));
                return Err(anyhow::anyhow!("Failed to check Python availability"));
            }
            _ => {}
        }

        // Check if poml module is available
        let poml_check = Command::new("python")
            .arg("-m")
            .arg("poml")
            .arg("--help")
            .output()
            .await;

        match poml_check {
            Ok(output) if output.status.success() => {
                let _ = self.tx.send(AppEvent::Log("poml-cli is available".to_string()));
                Ok(())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let _ = self.tx.send(AppEvent::Log(format!("Error: poml-cli module not available: {}", stderr)));
                Err(anyhow::anyhow!("poml-cli module not available"))
            }
            Err(e) => {
                let _ = self.tx.send(AppEvent::Log(format!("Error: Failed to check poml-cli: {}", e)));
                Err(anyhow::anyhow!("Failed to check poml-cli availability"))
            }
        }
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
