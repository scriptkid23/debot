use actix::prelude::*;
use debot::{
    consensus::actor::{ Consensus, SetNetworkAddr },
    network::actor::{ Network, SetConsensusAddr },
};

#[actix_rt::main]
async fn main() {
    // Start the network actor
    let network = Network::default().start();

    let consensus = Consensus::default().start();

    consensus.do_send(SetNetworkAddr(network.clone()));

    network.do_send(SetConsensusAddr(consensus.clone()));

    // Keep the main task running
    println!("Press Ctrl+C to exit");
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
}
