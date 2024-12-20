use std::time::Duration;

use actix::prelude::*;
use futures::StreamExt;
use libp2p::{noise, tcp, yamux, PeerId, Swarm};

use crate::consensus::actor::Consensus;

use super::behaviour::Behaviour;

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

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetConsensusAddr(pub Addr<Consensus>);

/// Our Actor that manages the libp2p swarm.
pub struct Network {
    swarm: Option<Swarm<Behaviour>>,
    consensus_addr: Option<Addr<Consensus>>,
}

impl Actor for Network {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let id_keys = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());

        let behaviour = Behaviour::new(&id_keys).expect("Failed to create behaviour");

        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .expect("msg")
            .with_behaviour(|_| behaviour)
            .expect("msg")
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        // Just run listen_on
        if let Err(e) = swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().expect("Invalid multiaddr")) {
            eprintln!("Failed to start listening: {:?}", e);
        } else {
            println!("Listening on all interfaces on a random port");
        }
        // Handle swarm events
        ctx.spawn(
            async move {
                loop {
                    if let Some(event) = swarm.next().await {
                        match event {
                            libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                                println!("Listening on {:?}", address);
                            }
                            _ => {}
                        }
                    }
                }
            }
            .into_actor(self),
        );
    }
}

impl Handler<SendMessage> for Network {
    type Result = ();
    fn handle(&mut self, msg: SendMessage, ctx: &mut Self::Context) -> Self::Result {
        print!("{:?}", msg.0);
    }
}

impl Handler<SetConsensusAddr> for Network {
    type Result = ();

    fn handle(&mut self, msg: SetConsensusAddr, _ctx: &mut Self::Context) -> Self::Result {
        self.consensus_addr = Some(msg.0);
    }
}

impl Default for Network {
    fn default() -> Self {
        Network {
            swarm: None,
            consensus_addr: None,
        }
    }
}

impl Network {}
