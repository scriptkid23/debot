use super::{CommandHandler, CommandResult};

/// Status command - shows the status of the current bot/node
pub struct StatusCommand;

impl CommandHandler for StatusCommand {
    fn name(&self) -> &str {
        "status"
    }

    fn execute(&self, _args: Vec<String>) -> CommandResult {
        // Status retrieval is handled by the actor layer
        // This is a placeholder that will be filled by the actor
        tracing::debug!("Executing status command");

        CommandResult::Success("Status check requested".to_string())
    }

    fn help(&self) -> String {
        "Show the current status of this bot and node.\nUsage: /status".to_string()
    }
}
