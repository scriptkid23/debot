use actix::prelude::*;
use debot::{
    client::{
        commands::parse_command, BroadcastCommand, Client, CommandRegistry, SetConsensusAddr,
        SubmitTransaction,
    },
    config::{Config, RaftConfig},
    consensus::actor::{Consensus, InitializeConsensus, SetNetworkAddr},
    network::actor::{Network, SetConsensusAddr as SetNetworkConsensusAddr},
};
use std::path::PathBuf;
use teloxide::prelude::*;
use teloxide::types::Message as TelegramMessage;
use tracing_subscriber;

#[actix_rt::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("🤖 Starting Debot Node...");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let bot_token = parse_token_arg(&args)?;

    tracing::info!("Connecting to Telegram bot...");

    // Create Telegram bot
    let bot = Bot::new(&bot_token);

    // Get bot info
    let me = bot.get_me().await?;
    let bot_username = me.username.clone().unwrap_or_else(|| "unknown".to_string());

    tracing::info!("✅ Bot connected: @{}", bot_username);

    // Create node configuration
    let node_id = format!("bot-{}-{}", bot_username, std::process::id());
    let raft_config = Config {
        raft: RaftConfig {
            node_id: node_id.clone(),
            election_timeout_min_ms: 150,
            election_timeout_max_ms: 300,
            heartbeat_interval_ms: 50,
            data_dir: PathBuf::from(format!("./data/{}", node_id)),
        },
        ..Default::default()
    };

    tracing::info!("Node ID: {}", raft_config.raft.node_id);

    // Start the network actor
    let network = Network::default().start();

    // Start the consensus actor
    let consensus = Consensus::default().start();

    // Create client actor for this bot
    let client = Client::new(bot_username.clone()).start();

    // Wire up the actors
    consensus.do_send(SetNetworkAddr(network.clone()));
    network.do_send(SetNetworkConsensusAddr(consensus.clone()));
    client.do_send(SetConsensusAddr(consensus.clone()));
    consensus.do_send(debot::consensus::actor::SetClientAddr(client.clone()));

    tracing::info!("✅ All actors wired up successfully");

    // Initialize consensus
    consensus.do_send(InitializeConsensus {
        config: raft_config.raft,
    });

    tracing::info!(
        "🚀 Bot @{} is ready! Listening for commands...",
        bot_username
    );
    tracing::info!("💡 Available commands: /send, /status");

    // Start bot event loop
    let registry = CommandRegistry::with_defaults();
    run_bot_loop(bot, client, bot_username, registry).await?;

    tracing::info!("👋 Shutting down...");
    Ok(())
}

fn parse_token_arg(args: &[String]) -> Result<String, String> {
    for arg in args.iter().skip(1) {
        if arg.starts_with("--token=") {
            let token = arg.strip_prefix("--token=").unwrap();
            if token.is_empty() {
                return Err("Bot token cannot be empty".to_string());
            }
            return Ok(token.to_string());
        }
    }

    // Try environment variable as fallback
    if let Ok(token) = std::env::var("TELEGRAM_BOT_TOKEN") {
        return Ok(token);
    }

    Err("Bot token not provided. Use: cargo run -- --token=YOUR_BOT_TOKEN\nOr set TELEGRAM_BOT_TOKEN environment variable".to_string())
}

async fn run_bot_loop(
    bot: Bot,
    client: Addr<Client>,
    bot_username: String,
    registry: CommandRegistry,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let handler = Update::filter_message().endpoint(move |bot: Bot, msg: TelegramMessage| {
        let client = client.clone();
        let bot_username = bot_username.clone();
        let registry = registry.clone();

        async move {
            if let Err(e) = handle_message(&bot, &client, &bot_username, msg, &registry).await {
                tracing::error!("Error handling message: {}", e);
            }
            respond(())
        }
    });

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn handle_message(
    bot: &Bot,
    client: &Addr<Client>,
    bot_username: &str,
    msg: TelegramMessage,
    registry: &CommandRegistry,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(text) = msg.text() {
        tracing::info!("📨 @{} received: {}", bot_username, text);

        // Try to parse as command
        if let Some((command, args)) = parse_command(text, "/") {
            // Check if command exists
            if !registry.has_command(&command) {
                let response = format!("❌ Unknown command: {}", command);
                bot.send_message(msg.chat.id, response).await?;
                return Ok(());
            }

            tracing::info!(
                "🎯 @{} executing command: {} {:?}",
                bot_username,
                command,
                args
            );

            // Create broadcast command
            let broadcast_cmd = BroadcastCommand {
                command: command.clone(),
                args: args.clone(),
                from_bot: bot_username.to_string(),
            };

            // Serialize and submit to consensus
            let data = bincode::serialize(&broadcast_cmd)
                .map_err(|e| format!("Failed to serialize command: {}", e))?;

            let tx = SubmitTransaction {
                bot_id: bot_username.to_string(),
                data,
            };

            match client.send(tx).await {
                Ok(Ok(tx_id)) => {
                    // Execute command locally and get response
                    let result = registry.execute(&command, args);
                    let response = match result {
                        Some(debot::client::CommandResult::Success(ref msg)) => {
                            format!("✅ {}\n📡 Broadcast TX: {}", msg, tx_id)
                        }
                        Some(debot::client::CommandResult::Error(ref err)) => {
                            format!("❌ Error: {}", err)
                        }
                        None => "❌ Unknown command".to_string(),
                    };

                    bot.send_message(msg.chat.id, response).await?;
                }
                Ok(Err(e)) => {
                    let response = format!("❌ Failed to broadcast: {}", e);
                    bot.send_message(msg.chat.id, response).await?;
                }
                Err(e) => {
                    let response = format!("❌ Internal error: {}", e);
                    bot.send_message(msg.chat.id, response).await?;
                }
            }
        } else {
            // Not a command, send help
            let commands = registry.list_commands();
            let response = format!(
                "Available commands:\n{}",
                commands
                    .iter()
                    .map(|c| format!("/{}", c))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            bot.send_message(msg.chat.id, response).await?;
        }
    }

    Ok(())
}
