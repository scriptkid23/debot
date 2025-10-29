use actix::prelude::*;
use debot::{
    config::{Config, RaftConfig},
    consensus::actor::{Consensus, InitializeConsensus, SetNetworkAddr},
    network::actor::{Network, SetConsensusAddr},
};
use std::path::PathBuf;
use tracing_subscriber;

#[actix_rt::main]
async fn main() {
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

    // Keep the main task running
    tracing::info!("Node started. Press Ctrl+C to exit");
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl+c");

    tracing::info!("Shutting down...");
}
