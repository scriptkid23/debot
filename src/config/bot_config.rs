use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    #[serde(default)]
    pub cluster: ClusterConfig,
    pub bots: Vec<BotInfo>,
    #[serde(default)]
    pub commands: CommandConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    #[serde(default = "default_node_id_prefix")]
    pub node_id_prefix: String,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            node_id_prefix: default_node_id_prefix(),
        }
    }
}

fn default_node_id_prefix() -> String {
    "bot-node".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotInfo {
    pub name: String,
    pub token: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    #[serde(default = "default_prefix")]
    pub prefix: String,
    #[serde(default)]
    pub allowed: Vec<String>,
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            prefix: default_prefix(),
            allowed: vec![],
        }
    }
}

fn default_prefix() -> String {
    "/".to_string()
}

impl BotConfig {
    /// Load bot configuration from a TOML file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let contents =
            fs::read_to_string(path).map_err(|e| format!("Failed to read config file: {}", e))?;

        let config: BotConfig =
            toml::from_str(&contents).map_err(|e| format!("Failed to parse config file: {}", e))?;

        // Validate
        if config.bots.is_empty() {
            return Err("No bots configured".to_string());
        }

        Ok(config)
    }

    /// Get all enabled bots
    pub fn enabled_bots(&self) -> Vec<&BotInfo> {
        self.bots.iter().filter(|b| b.enabled).collect()
    }

    /// Check if a command is allowed
    pub fn is_command_allowed(&self, command: &str) -> bool {
        if self.commands.allowed.is_empty() {
            // If no allowed list, allow all
            return true;
        }
        self.commands.allowed.contains(&command.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let toml_str = r#"
[cluster]
node_id_prefix = "test-node"

[[bots]]
name = "bot1"
token = "token1"
enabled = true

[[bots]]
name = "bot2"
token = "token2"
enabled = false

[commands]
prefix = "/"
allowed = ["send", "status"]
"#;

        let config: BotConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.bots.len(), 2);
        assert_eq!(config.enabled_bots().len(), 1);
        assert_eq!(config.cluster.node_id_prefix, "test-node");
        assert!(config.is_command_allowed("send"));
        assert!(!config.is_command_allowed("unknown"));
    }
}
