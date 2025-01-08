use super::codec::MessageCodec;
use libp2p::{
    mdns,
    request_response::{self},
    swarm::NetworkBehaviour,
};

#[derive(NetworkBehaviour)]
pub struct Behaviour {
    pub mdns: mdns::tokio::Behaviour,
    pub request_response: request_response::Behaviour<MessageCodec>,
}
