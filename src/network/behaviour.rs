use super::codec::MessageCodec;
use libp2p::{
    mdns,
    request_response::{self, Config, ProtocolSupport},
    swarm::NetworkBehaviour,
};
use std::error::Error;

#[derive(NetworkBehaviour)]
pub struct Behaviour {
    pub mdns: mdns::tokio::Behaviour,
    pub request_response: request_response::Behaviour<MessageCodec>,
}

impl Behaviour {
    pub fn new(key: &libp2p::identity::Keypair) -> Result<Self, Box<dyn Error>> {
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;

        let protocols = vec![("/message_protocol/1", ProtocolSupport::Full)];

        let config = Config::default();

        let request_response = request_response::Behaviour::new(protocols, config);

        Ok(Behaviour {
            mdns,
            request_response,
        })
    }
}
