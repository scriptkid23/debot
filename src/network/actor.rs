use std::{collections::HashMap, time::Duration};

use actix::prelude::*;
use anyhow::Ok;
use libp2p::{noise, tcp, yamux, PeerId, Swarm};

use crate::consensus::actor::Consensus;

use super::behaviour::{Behaviour, BehaviourEvent};

/// Messages to send to the actor
#[derive(Message)]
#[rtype(result = "()")]
pub struct SendMessage(pub String);

/// Events the actor can produce
#[derive(Message)]
#[rtype(result = "()")]
pub enum NetworkEvent {
    DiscoveredPeer(PeerId),
    ReceivedRequest { from: PeerId, msg: String },
    ReceivedResponse { from: PeerId, msg: String },
    OutboundFailure { peer: PeerId },
    InboundFailure { peer: PeerId },
}

/// Requesting the swarm from the actor (for internal use)
struct GetSwarm;

impl Message for GetSwarm {
    type Result = Swarm<Behaviour>;
}

/// A message carrying a `SwarmEvent`
struct SwarmEventMessage(libp2p::swarm::SwarmEvent<BehaviourEvent>);

impl Message for SwarmEventMessage {
    type Result = ();
}

/// Our Actor that manages the libp2p swarm.
pub struct Network {
    peer_ids: Vec<PeerId>,
}

impl Actor for Network {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("Actor is alive");
    }
}

impl Handler<SendMessage> for Network {
    type Result = ();
    fn handle(&mut self, msg: SendMessage, ctx: &mut Self::Context) -> Self::Result {
        print!("{:?}", msg.0);
    }
}

impl Default for Network {
    fn default() -> Self {
        Network {
            peer_ids: Vec::new(),
        }
    }
}

impl Network {
    pub fn new() {
        let id_keys = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());

        println!("Local peer id: {peer_id}");

        // let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        //     .with_tokio()
        //     .with_tcp(
        //         tcp::Config::default(),
        //         noise::Config::new,
        //         yamux::Config::default,
        //     )?
        //     .with_behaviour(Behaviour::new(&key))?
        //     .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        //     .build();

        // Ok(Libp2pActor { id_keys, swarm })
    }
}
