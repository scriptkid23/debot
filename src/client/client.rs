use actix::prelude::*;
use teloxide::{prelude::*, types::Message as TelegramMessage, utils::command::BotCommands};
use tokio::sync::mpsc;

use crate::consensus::actor::{BroadcastTest, Consensus, IsLeader};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Debot commands:")]
enum Command {
    #[command(description = "Check if node is leader or follower")]
    Test,
}

/// Spawns a Telegram bot that listens for commands and responds based on node role
/// Returns a sender that can be used to send chat_ids for test responses
pub fn spawn_telegram_bot(
    token: String,
    consensus_addr: Addr<Consensus>,
) -> mpsc::UnboundedSender<i64> {
    let (test_tx, mut test_rx) = mpsc::unbounded_channel::<i64>();

    let bot_for_test = Bot::new(token.clone());

    // Spawn a task to handle test notifications from other nodes
    tokio::spawn(async move {
        while let Some(chat_id) = test_rx.recv().await {
            tracing::info!("🔔 Received test notification for chat_id: {}", chat_id);
            if let Err(e) = bot_for_test
                .send_message(ChatId(chat_id), "I am a follower")
                .await
            {
                tracing::error!("Failed to send follower message: {:?}", e);
            }
        }
    });

    // Spawn the main bot handler
    tokio::spawn(async move {
        tracing::info!("🤖 Starting Telegram bot...");

        let bot = Bot::new(token);

        // Set up command handler
        let handler = Update::filter_message()
            .filter_command::<Command>()
            .endpoint(handle_command);

        let consensus = consensus_addr.clone();

        Dispatcher::builder(bot, handler)
            .dependencies(dptree::deps![consensus])
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;
    });

    test_tx
}

async fn handle_command(
    bot: Bot,
    msg: TelegramMessage,
    cmd: Command,
    consensus: Addr<Consensus>,
) -> ResponseResult<()> {
    match cmd {
        Command::Test => {
            tracing::info!("📨 Received /test command from chat_id: {}", msg.chat.id);

            // Check if this node is the leader
            match consensus.send(IsLeader).await {
                Ok(is_leader) => {
                    if is_leader {
                        // Reply as leader
                        bot.send_message(msg.chat.id, "I am a leader").await?;
                        tracing::info!("👑 Replied as leader");

                        // Broadcast to all other nodes
                        consensus.do_send(BroadcastTest {
                            chat_id: msg.chat.id.0,
                        });
                        tracing::info!("📡 Broadcast test message to followers");
                    } else {
                        // Reply as follower
                        bot.send_message(msg.chat.id, "I am a follower").await?;
                        tracing::info!("👥 Replied as follower");
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to query leader status: {:?}", e);
                    bot.send_message(msg.chat.id, "Error checking node status")
                        .await?;
                }
            }
        }
    }

    Ok(())
}
