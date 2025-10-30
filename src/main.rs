use actix::prelude::*;
use clap::Parser;
use debot::{
    client::spawn_telegram_bot,
    config::{Config, RaftConfig},
    consensus::actor::{Consensus, InitializeConsensus, SetNetworkAddr},
    network::actor::{Network, SetConsensusAddr},
};
use std::path::PathBuf;
use tracing_subscriber;

#[derive(Parser, Debug)]
#[command(name = "debot")]
#[command(about = "Debot - A decentralized bot system with Raft consensus", long_about = None)]
struct Args {
    /// Telegram bot token (optional)
    #[arg(long)]
    token: Option<String>,
}

#[actix_rt::main]
async fn main() {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting Debot node...");

    // Create configuration
    let config = Config {
        raft: RaftConfig {
            node_id: format!("node-{}", std::process::id()),
            election_timeout_min_ms: 150,
            election_timeout_max_ms: 300,
            heartbeat_interval_ms: 50,
            data_dir: PathBuf::from("./data"),
        },
        ..Default::default()
    };

    tracing::info!("Node ID: {}", config.raft.node_id);

    // Start the network actor
    let network = Network::default().start();

    let consensus = Consensus::default().start();

    // Wire up the actors
    consensus.do_send(SetNetworkAddr(network.clone()));
    network.do_send(SetConsensusAddr(consensus.clone()));

    // Initialize consensus with config
    consensus.do_send(InitializeConsensus {
        config: config.raft,
    });

    // Start Telegram bot if token is provided
    if let Some(token) = args.token {
        tracing::info!("🤖 Telegram bot token provided, starting bot...");
        let bot_sender = spawn_telegram_bot(token, consensus.clone());

        // Register bot sender with consensus
        consensus.do_send(debot::consensus::actor::SetBotTestSender { sender: bot_sender });
    } else {
        tracing::info!("ℹ️  No Telegram bot token provided, running without bot");
    }

    // Keep the main task running
    tracing::info!("Node started. Press Ctrl+C to exit");
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl+c");

    tracing::info!("Shutting down...");
}
