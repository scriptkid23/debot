use super::{CommandHandler, CommandResult};

/// Send command - broadcasts a message to all bots in the cluster
pub struct SendCommand;

impl CommandHandler for SendCommand {
    fn name(&self) -> &str {
        "send"
    }

    fn execute(&self, args: Vec<String>) -> CommandResult {
        if args.is_empty() {
            return CommandResult::Error("Usage: /send <message>".to_string());
        }

        let message = args.join(" ");

        // The actual broadcasting is handled by the actor layer
        // This just validates and formats the command
        tracing::info!("Executing send command: {}", message);

        CommandResult::Success(format!("sending"))
    }

    fn help(&self) -> String {
        "Send a message that will be broadcast to all bots in the cluster.\nUsage: /send <message>"
            .to_string()
    }
}
