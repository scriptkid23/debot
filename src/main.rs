use debot::network::libp2p;

#[actix::main]
async fn main() {
    // You need to await the startup function to execute the future it returns
    if let Err(e) = libp2p::startup().await {
        eprintln!("Error starting up libp2p: {}", e);
    }
}
