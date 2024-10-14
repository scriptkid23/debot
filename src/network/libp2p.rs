use actix::Message;
use libp2p::kad::{store::MemoryStore, Behaviour as KademliaBehaviour};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{gossipsub::Behaviour as GossipsubBehaviour, mdns::tokio::Behaviour as MdnsBehaviour};
use libp2p::{PeerId, Swarm};
use tokio::sync::mpsc;

pub struct NetworkActor {
    swarm: Swarm<MyBehaviour>,
    msg_sender: mpsc::Sender<NetworkMessage>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct NetworkMessage {
    pub data: Vec<u8>,
}

// / Defines the network behavior combining Kademlia, mDNS, and Gossipsub.
#[derive(NetworkBehaviour)]
pub struct MyBehaviour {
    pub kademlia: KademliaBehaviour<MemoryStore>,
    pub mdns: MdnsBehaviour,
    pub gossipsub: GossipsubBehaviour,
}

impl MyBehaviour {
    pub fn new(local_peer_id: PeerId) -> Result<Self, Box<dyn Error>> {
        let store = MemoryStore::new(local_peer_id.clone());

        Ok(MyBehaviour {})
    }
}
