use crate::runner::AppEvent;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;
use anyhow::Result;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tokio::fs;

/// Represents an NMMCP (NeonMachines Model Control Protocol) extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NMMCPExtension {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub entry_point: PathBuf,
    pub dependencies: Vec<String>,
    pub tools: Vec<ExtensionTool>,
    pub capabilities: ExtensionCapabilities,
}

/// Represents a tool provided by an extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionTool {
    pub name: String,
    pub description: String,
    pub parameters: ExtensionParameters,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
}

/// Parameters for an extension tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionParameters {
    pub required: Vec<String>,
    pub optional: Vec<String>,
    pub types: HashMap<String, String>,
}

/// Capabilities of an extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionCapabilities {
    pub model_control: bool,
    pub tool_integration: bool,
    pub file_operations: bool,
    pub system_access: bool,
}

/// Registry for managing NMMCP extensions
pub struct NMMCPExtensionRegistry {
    extensions: HashMap<String, NMMCPExtension>,
    tx: UnboundedSender<AppEvent>,
}

impl NMMCPExtensionRegistry {
    pub fn new(tx: UnboundedSender<AppEvent>) -> Self {
        Self {
            extensions: HashMap::new(),
            tx,
        }
    }

    /// Load an NMMCP extension from a directory
    pub async fn load_extension(&mut self, extension_dir: &PathBuf) -> Result<()> {
        if !extension_dir.exists() {
            let _ = self.tx.send(AppEvent::Log(format!("Error: Extension directory not found: {}", extension_dir.display())));
            return Ok(());
        }

        // Look for extension metadata file
        let metadata_file = extension_dir.join("nmmcp.json");
        if !metadata_file.exists() {
            let _ = self.tx.send(AppEvent::Log(format!("Error: No nmmcp.json found in: {}", extension_dir.display())));
            return Ok(());
        }

        // Load metadata
        let metadata_content = fs::read_to_string(&metadata_file).await?;
        let extension: NMMCPExtension = serde_json::from_str(&metadata_content)?;

        // Validate extension entry point
        let entry_point = extension_dir.join(&extension.entry_point);
        if !entry_point.exists() {
            let _ = self.tx.send(AppEvent::Log(format!("Error: Entry point not found: {}", entry_point.display())));
            return Ok(());
        }

        // Store the extension
        self.extensions.insert(extension.name.clone(), extension.clone());
        let _ = self.tx.send(AppEvent::Log(format!("Loaded extension: {}", extension.name)));

        Ok(())
    }

    /// Load all extensions from a directory
    pub async fn load_extensions_from_directory(&mut self, extensions_dir: &PathBuf) -> Result<()> {
        if !extensions_dir.exists() {
            let _ = self.tx.send(AppEvent::Log(format!("Extensions directory not found: {}", extensions_dir.display())));
            return Ok(());
        }

        // Look for extension directories
        let mut entries = fs::read_dir(extensions_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("ext_") || name.starts_with("nmmcp_") {
                        let _ = self.tx.send(AppEvent::Log(format!("Found extension directory: {}", name)));
                        if let Err(e) = self.load_extension(&path).await {
                            let _ = self.tx.send(AppEvent::Log(format!("Failed to load extension {}: {}", name, e)));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get all loaded extensions
    pub fn get_extensions(&self) -> &HashMap<String, NMMCPExtension> {
        &self.extensions
    }

    /// Get a specific extension by name
    pub fn get_extension(&self, name: &str) -> Option<&NMMCPExtension> {
        self.extensions.get(name)
    }

    /// List all available extension tools
    pub fn list_all_tools(&self) -> Vec<(String, String)> {
        let mut tools = Vec::new();
        for (ext_name, extension) in &self.extensions {
            for tool in &extension.tools {
                tools.push((format!("{}:{}", ext_name, tool.name), tool.description.clone()));
            }
        }
        tools
    }

    /// Check if an extension supports a specific capability
    pub fn supports_capability(&self, extension_name: &str, capability: &str) -> bool {
        self.extensions.get(extension_name)
            .map(|ext| match capability {
                "model_control" => ext.capabilities.model_control,
                "tool_integration" => ext.capabilities.tool_integration,
                "file_operations" => ext.capabilities.file_operations,
                "system_access" => ext.capabilities.system_access,
                _ => false,
            })
            .unwrap_or(false)
    }

    /// Uninstall an extension
    pub async fn uninstall_extension(&mut self, name: &str) -> Result<()> {
        if self.extensions.remove(name).is_some() {
            let _ = self.tx.send(AppEvent::Log(format!("Uninstalled extension: {}", name)));
            Ok(())
        } else {
            Err(anyhow::anyhow!("Extension not found: {}", name))
        }
    }
}

/// Built-in NMMCP extensions directory
pub fn get_extensions_directory() -> PathBuf {
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home_dir.join(".neonmachines").join("extensions")
}

/// Default extension directories to search
pub fn get_default_extension_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    
    // User extensions directory
    dirs.push(get_extensions_directory());
    
    // System extensions directory
    dirs.push(PathBuf::from("/usr/local/lib/neonmachines/extensions"));
    
    // Current directory extensions
    dirs.push(PathBuf::from("./extensions"));
    
    dirs
}

/// Load all available extensions from default directories
pub async fn load_all_extensions(tx: UnboundedSender<AppEvent>) -> Result<NMMCPExtensionRegistry> {
    let mut registry = NMMCPExtensionRegistry::new(tx.clone());
    
    let directories = get_default_extension_directories();
    for dir in directories {
        if dir.exists() {
            let _ = tx.send(AppEvent::Log(format!("Searching for extensions in: {}", dir.display())));
            if let Err(e) = registry.load_extensions_from_directory(&dir).await {
                let _ = tx.send(AppEvent::Log(format!("Failed to load extensions from {}: {}", dir.display(), e)));
            }
        }
    }
    
    let count = registry.get_extensions().len();
    let _ = tx.send(AppEvent::Log(format!("Loaded {} extensions", count)));
    
    Ok(registry)
}

/// Validate extension metadata
pub fn validate_extension_metadata(metadata: &NMMCPExtension) -> Result<()> {
    if metadata.name.is_empty() {
        return Err(anyhow::anyhow!("Extension name cannot be empty"));
    }
    
    if metadata.version.is_empty() {
        return Err(anyhow::anyhow!("Extension version cannot be empty"));
    }
    
    if metadata.entry_point.components().count() == 0 {
        return Err(anyhow::anyhow!("Entry point cannot be empty"));
    }
    
    if metadata.tools.is_empty() {
        return Err(anyhow::anyhow!("Extension must have at least one tool"));
    }
    
    // Validate tools
    for tool in &metadata.tools {
        if tool.name.is_empty() {
            return Err(anyhow::anyhow!("Tool name cannot be empty"));
        }
        if tool.description.is_empty() {
            return Err(anyhow::anyhow!("Tool description cannot be empty"));
        }
        
        // Check required parameters exist in input schema
        let schema = &tool.input_schema;
        if let Some(obj) = schema.as_object() {
            for param in &tool.parameters.required {
                if !obj.contains_key(param) {
                    return Err(anyhow::anyhow!("Required parameter '{}' not found in input schema", param));
                }
            }
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_extension_registry_creation() {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let registry = NMMCPExtensionRegistry::new(tx);
        assert!(registry.get_extensions().is_empty());
    }

    #[tokio::test]
    async fn test_extension_directories() {
        let dirs = get_default_extension_directories();
        assert!(dirs.len() >= 3);
        
        for dir in dirs {
            println!("Extension directory: {}", dir.display());
        }
    }
}
