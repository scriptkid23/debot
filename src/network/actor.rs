use std::{collections::HashSet, time::Duration};

use actix::prelude::*;
use futures::{channel::mpsc, StreamExt};
use libp2p::{mdns, noise, request_response, swarm::SwarmEvent, tcp, yamux, Multiaddr, PeerId, Swarm};
use tokio::{select};
use tokio_util::codec::{FramedRead, LinesCodec};
use crate::{consensus::actor::Consensus, network::{behaviour::BehaviourEvent, codec::{MessageRequest, MessageResponse}}};

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

#[derive(Message)]
#[rtype(result = "()")]
pub struct NetworkMessage(pub String);

enum SwarmCommand {
    SendMessage(String),
    // Thêm các lệnh khác nếu cần
}

/// Our Actor that manages the libp2p swarm.
pub struct Network {
    swarm: Option<Swarm<Behaviour>>,
    command_sender: Option<mpsc::Sender<SwarmCommand>>,
    consensus_addr: Option<Addr<Consensus>>,
}

async fn try_dial_peer(swarm: &mut Swarm<Behaviour>, peer_address: Multiaddr) {
    if let Err(e) = swarm.dial(peer_address.clone()) {
        println!("Dialing failed: {e}");
    } else {
        println!("Successfully dialed peer: {peer_address}");
    }
}

impl Actor for Network {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("Started network listener");

        let mut swarm = self.swarm.take().expect("Swarm should be initialized");

        if let Err(e) = swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().expect("Invalid multiaddr")) {
            eprintln!("Failed to start listening: {:?}", e);
        } else {
            println!("Listening on all interfaces on a random port");
        }

        let (command_sender, mut command_receiver) = mpsc::channel(32);

        self.command_sender = Some(command_sender);

        let stdin = tokio::io::stdin();
        let mut stdin = FramedRead::new(stdin, LinesCodec::new());

        println!("Type 'send <message>' to send a message to peers:");
        
        ctx.spawn(
            async move {
                let mut pending_requests = Vec::new();
                loop {
                    select! {
                      
                        event = swarm.select_next_some() => match event {
                            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                                for (peer_id, multiaddr) in list {
                                    println!("mDNS discovered a new peer: {peer_id}");
                                    if !swarm.is_connected(&peer_id) {
                                        try_dial_peer(&mut swarm, multiaddr).await;
                                    }
                                }
                            },

                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                println!("Connection established with {peer_id}");
                                
                                pending_requests.retain(|(target_peer, message): &(PeerId, String)| {
                                    if *target_peer == peer_id {
                                        println!("Sending message to {peer_id}: {message}");
                                        let request = MessageRequest(message.clone());
                                        swarm.behaviour_mut().request_response.send_request(target_peer, request);
                                        false 
                                    } else {
                                        true 
                                    }
                                });

                               
                            }
            
                            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                                for (peer_id, _multiaddr) in list {
                                    println!("mDNS discover peer has expired: {peer_id}");
                                }
                            },
            
                            SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(event)) => {
                                match event {
                                    request_response::Event::Message { peer, message } => {
                                        match message {
                                            request_response::Message::Request { request, channel, .. } => {
                                                println!("Received request from {}: {:?}", peer, request);
            
                                                let response = MessageResponse("Response to your request".to_string());
                                                swarm.behaviour_mut().request_response.send_response(channel, response).unwrap();
                                            }
                                            request_response::Message::Response { request_id, response } => {
                                                println!("Received response for request {}: {:?}", request_id, response);
            
                                            }
                                        }
                                    }
                                    request_response::Event::OutboundFailure { peer, request_id, error } => {
                                        println!("Outbound failure for request {} to {}: {:?}", request_id, peer, error);
                                    }
                                    request_response::Event::InboundFailure { peer, request_id, error } => {
                                        println!("Inbound failure from {} for request {}: {:?}", peer, request_id, error);
                                    }
                                    request_response::Event::ResponseSent { peer, request_id } => {
                                        println!("Response sent to {} for request {}", peer, request_id);
                                    }
                                }
                            },
            
                            SwarmEvent::NewListenAddr { address, .. } => {
                                println!("Local node is listening on {address}");
                            }
                            _ => {}
                        },

                        line = stdin.next() => match line {
                            Some(Ok(line)) => {
                                if let Some(message) = line.strip_prefix("send ") {
                                    println!("Sending...: {}", message);
                                    let peers: HashSet<PeerId> = swarm.behaviour().mdns.discovered_nodes().cloned().collect();
            
                                    println!("{:?}", peers);
            
            
                                    if peers.is_empty() {
                                            print!("Nothing!")
                                    }else {
                                        for peer in peers {
                                            if swarm.is_connected(&peer) {
                                                println!("Sending message to connected peer: {peer}");
                                                let request = MessageRequest(message.to_string());
                                                swarm.behaviour_mut().request_response.send_request(&peer, request);
                                            } else {
                                                println!("Peer {peer} not connected, queuing message");
                                                pending_requests.push((peer.clone(), message.to_string()));
                                            }
                                        }
                                    }
                                }
                            }
                            Some(Err(e)) => eprintln!("Error reading from stdin: {:?}", e),
                            None => break,
                        },
                        
                       
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
        println!("Received SendMessage: {:?}", msg.0);
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
        let id_keys = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());

        println!("Local peer id: {peer_id}");

        let behaviour = Behaviour::new(&id_keys).expect("Failed to create behaviour");
        let swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .expect("Error")
            .with_behaviour(|_| behaviour)
            .expect("Error")
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        Network {
            swarm: Some(swarm),
            command_sender: None,
            consensus_addr: None,
        }
    }
}

impl Handler<NetworkMessage> for Network {
    type Result = ();

    fn handle(&mut self, msg: NetworkMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(sender) = &mut self.command_sender {
            if let Err(e) = sender.try_send(SwarmCommand::SendMessage(msg.0)) {
                println!("Failed to send message to swarm: {:?}", e);
            }
        } else {
            println!("Command sender not initialized");
        }
    }
}

impl Network {}
