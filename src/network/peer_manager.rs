// src/network/peer_manager.rs
use actix::prelude::*;
use libp2p::{PeerId, Multiaddr};
use crate::network::NetworkMessage;

pub struct PeerManager {
    pub peers: Vec<PeerId>,
}

impl PeerManager {
    pub fn add_peer(&mut self, peer_id: PeerId) {
        if !self.peers.contains(&peer_id) {
            self.peers.push(peer_id);
            println!("Added peer: {:?}", peer_id);
        }
    }
}

impl Actor for PeerManager {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        println!("Peer manager started");
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct AddPeer(pub PeerId);

impl Handler<AddPeer> for PeerManager {
    type Result = ();

    fn handle(&mut self, msg: AddPeer, _: &mut Self::Context) {
        self.add_peer(msg.0);
    }
}

impl Handler<NetworkMessage> for PeerManager {
    type Result = ();

    fn handle(&mut self, msg: NetworkMessage, _: &mut Self::Context) {
        // Gửi tin nhắn đến tất cả các peer
        for peer_id in &self.peers {
            println!("Sending message to {:?}", peer_id);
            // Thực thi logic gửi tin nhắn tới từng peer tại đây
        }
    }
}
