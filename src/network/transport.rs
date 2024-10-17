use libp2p::{
    core::{muxing::StreamMuxerBox, transport::Boxed, upgrade::Version},
    dns::DnsConfig,
    identity::Keypair,
    mplex, noise::{self, NoiseConfig, X25519Spec, Keypair as NoiseKeypair},
    tcp::Config,
    websocket::WsConfig,
    yamux, Transport,
    Multiaddr,
    PeerId,
};
use std::error::Error;

/// Sets up the transport protocols (TCP, WebSocket) with authentication and multiplexing.
pub fn create_transport(local_keypair: Keypair) -> Result<Boxed<(PeerId, StreamMuxerBox)>, Box<dyn Error>> {
    // Generate a keypair for noise protocol (for encryption/authentication)
    let noise_keys = NoiseKeypair::<X25519Spec>::new().into_authentic(&local_keypair)?;

    // Set up TCP transport
    let tcp_transport = TcpConfig::new();

    // Set up WebSocket transport using TCP as a base layer
    let ws_transport = WsConfig::new(TcpConfig::new());

    // Combine TCP and WebSocket transports
    let base_transport = tcp_transport.or_transport(ws_transport);

    // Use DNS to resolve addresses
    let dns_transport = DnsConfig::new(base_transport)?;

    // Apply noise protocol for authentication and encryption
    let authenticated_transport = dns_transport
        .upgrade(Version::V1)
        .authenticate(NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(yamux::YamuxConfig::default()) // or mplex::MplexConfig::new()
        .boxed();

    Ok(authenticated_transport)
}
