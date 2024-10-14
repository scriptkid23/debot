use actix::prelude::*;
use libp2p::PeerId;

// Define the NetworkMessage struct
#[derive(Message)]
#[rtype(result = "()")]
pub struct NetworkMessage {
    pub peer_id: PeerId,
    pub data: Vec<u8>,
}
