use std::collections::HashMap;
use std::sync::Arc;

mod send_command;
mod status_command;

pub use send_command::SendCommand;
pub use status_command::StatusCommand;

/// Result of command execution
#[derive(Debug, Clone)]
pub enum CommandResult {
    Success(String),
    Error(String),
}

/// Trait for command handlers - implement this to add new commands
pub trait CommandHandler: Send + Sync {
    /// Get the command name (without prefix, e.g., "send" not "/send")
    fn name(&self) -> &str;

    /// Execute the command with given arguments
    fn execute(&self, args: Vec<String>) -> CommandResult;

    /// Get help text for this command
    fn help(&self) -> String {
        format!("No help available for {}", self.name())
    }
}

/// Registry for managing command handlers
#[derive(Clone)]
pub struct CommandRegistry {
    handlers: Arc<HashMap<String, Arc<dyn CommandHandler>>>,
}

impl CommandRegistry {
    /// Create a new empty command registry
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(HashMap::new()),
        }
    }

    /// Create a registry with default commands
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(SendCommand));
        registry.register(Arc::new(StatusCommand));
        registry
    }

    /// Register a command handler
    pub fn register(&mut self, handler: Arc<dyn CommandHandler>) {
        let handlers = Arc::make_mut(&mut self.handlers);
        handlers.insert(handler.name().to_string(), handler);
    }

    /// Execute a command by name with arguments
    pub fn execute(&self, command: &str, args: Vec<String>) -> Option<CommandResult> {
        self.handlers
            .get(command)
            .map(|handler| handler.execute(args))
    }

    /// Get help for a specific command
    pub fn get_help(&self, command: &str) -> Option<String> {
        self.handlers.get(command).map(|handler| handler.help())
    }

    /// List all registered commands
    pub fn list_commands(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// Check if a command exists
    pub fn has_command(&self, command: &str) -> bool {
        self.handlers.contains_key(command)
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Parse a command string into command name and arguments
/// Example: "/send ddos target" -> ("send", vec!["ddos", "target"])
pub fn parse_command(text: &str, prefix: &str) -> Option<(String, Vec<String>)> {
    let text = text.trim();

    if !text.starts_with(prefix) {
        return None;
    }

    let without_prefix = &text[prefix.len()..];
    let parts: Vec<&str> = without_prefix.split_whitespace().collect();

    if parts.is_empty() {
        return None;
    }

    let command = parts[0].to_lowercase();
    let args = parts[1..].iter().map(|s| s.to_string()).collect();

    Some((command, args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command() {
        assert_eq!(
            parse_command("/send ddos", "/"),
            Some(("send".to_string(), vec!["ddos".to_string()]))
        );

        assert_eq!(
            parse_command("/status", "/"),
            Some(("status".to_string(), vec![]))
        );

        assert_eq!(parse_command("not a command", "/"), None);
    }

    #[test]
    fn test_registry() {
        let registry = CommandRegistry::with_defaults();
        assert!(registry.has_command("send"));
        assert!(registry.has_command("status"));
        assert!(!registry.has_command("unknown"));
    }
}
