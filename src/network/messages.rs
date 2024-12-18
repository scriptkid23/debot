use actix::prelude::*;
use libp2p::PeerId;

// Define the NetworkMessage struct
#[derive(Message)]
#[rtype(result = "()")]
pub struct NetworkMessage {
    pub peer_id: PeerId,
    pub data: Vec<u8>,
}

#[derive(Message)]
#[rtype(result = "Responses")]
enum Messages {
    Ping,
    Pong,
}

enum Responses {
    GotPing,
    GotPong,
}
