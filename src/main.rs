use actix::prelude::*;
use debot::network::actor::Network;

#[actix_rt::main]
async fn main() {
    // Start the network actor
    let _network = Network::default().start();

    // Keep the main task running
    println!("Press Ctrl+C to exit");
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl+c");
}

