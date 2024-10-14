use libp2p::{
    core::upgrade,
    identity::{Keypair, PeerId},
    mplex::MplexConfig,
    noise::{NoiseConfig, X25519Spec},
    tcp::TcpConfig,
    websocket::WsConfig,
    yamux::YamuxConfig,
    Swarm, Transport,
};
use std::error::Error;
use tokio::io::AsyncWriteExt;

pub fn build_transport(local_keypair: Keypair) -> Result<impl Transport, Box<dyn Error>> {
    // Generate Noise keypair from libp2p identity Keypair
    let noise_keys = NoiseKeypair::<X25519Spec>::new()
        .into_authentic(&local_keypair)
        .map_err(|e| format!("Failed to create authentic keypair: {}", e))?;

    // TCP transport configuration
    let tcp_transport = TcpConfig::new();

    // WebSocket transport configuration
    let ws_transport = WsConfig::new(tcp_transport.clone());

    // Apply Noise (encryption) and Yamux (multiplexing) upgrades
    let authenticated_tcp_transport = tcp_transport
        .clone()
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(noise_keys.clone()))
        .multiplex(YamuxConfig::default())
        .boxed();

    let authenticated_ws_transport = ws_transport
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(noise_keys))
        .multiplex(MplexConfig::new())
        .boxed();

    // Combine both transports with fallback between TCP and WebSocket
    Ok(authenticated_tcp_transport.or(authenticated_ws_transport))
}

// Initialize and return a Swarm for managing peer connections
pub fn create_swarm(local_keypair: Keypair) -> Swarm {
    let local_peer_id = PeerId::from(local_keypair.public());
    let transport = build_transport(local_keypair).expect("Failed to create transport");

    // Set up the swarm with the specified transport
    Swarm::new(transport, local_peer_id)
}
