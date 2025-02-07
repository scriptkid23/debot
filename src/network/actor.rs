// Data Flow
// Step 1: A Response Layer receives incoming data from a node, creates a Network Event, and forwards it to the Network Layer.
// Step 2: The Network Layer processes the event and generates a Request Message, broadcasting it to other nodes.
// Step 3: The receiving node’s Request Layer captures the request and forwards it to the Response Layer.
// Step 4: The Response Layer processes the request, generates a response, and restarts the cycle.

use std::{ collections::HashSet, time::Duration, vec };

use crate::{
    consensus::actor::{ Consensus, Network2ConsensusRequest },
    network::{ self, behaviour::BehaviourEvent, codec::{ MessageRequest, MessageResponse } },
};
use actix::prelude::*;
use futures::{ channel::mpsc, StreamExt };
use libp2p::{
    mdns,
    noise,
    request_response::{ self, Config, ProtocolSupport },
    swarm::SwarmEvent,
    tcp,
    yamux,
    PeerId,
    Swarm,
};
use tokio::select;

use super::behaviour::Behaviour;

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetConsensusAddr(pub Addr<Consensus>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct ConsensusMessage(pub Vec<u8>);

enum SwarmCommand {
    ReceiveDataFrame {
        peer_ids: HashSet<PeerId>,
        msg: Vec<u8>,
    },
    Shutdown,
}

#[derive(Debug)]
pub enum NetworkError {
    ConnectionFailed,
    Timeout,
    InvalidData,
    // Add other error variants as needed
}

#[derive(Message)]
#[rtype(result = "Result<String, NetworkError>")]
pub struct NetWorkEvent(pub String);

#[derive(Message)]
#[rtype(result = "()")]
pub enum SwarmEventMessage {
    ConnectionEstablished(PeerId),
    ConnectionClosed(PeerId),
}

#[derive(Message)]
#[rtype(result = "Result<String, NetworkError>")]
pub enum NetworkEventMessage {
    Request(PeerId, String),
    Response(PeerId, String),
}

/// Our Actor that manages the libp2p swarm.
pub struct Network {
    peer_ids: HashSet<PeerId>,
    swarm: Option<Swarm<Behaviour>>,
    command_sender: Option<mpsc::Sender<SwarmCommand>>,
    consensus_addr: Option<Addr<Consensus>>,
}

impl Actor for Network {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("Started network listener");

        let mut swarm = self.swarm.take().expect("Swarm should be initialized");

        if
            let Err(e) = swarm.listen_on(
                "/ip4/0.0.0.0/tcp/0"
                    .parse()
                    .unwrap_or_else(|_| {
                        panic!("Invalid multiaddr provided. Check your configuration.")
                    })
            )
        {
            tracing::error!("Failed to start listening: {:?}", e);
        } else {
            tracing::info!("Listening on all interfaces on a random port");
        }

        let (command_sender, command_receiver) = mpsc::channel(32);

        self.command_sender = Some(command_sender);

        let network_addr = ctx.address();

        // let consensus_addr = match self.consensus_addr.clone() {
        //     Some(addr) => addr,
        //     None => {
        //         tracing::error!("Consensus address is missing");
        //         return;
        //     }
        // };

        ctx.spawn(
            (async move { run_swarm_loop(swarm, command_receiver, network_addr).await }).into_actor(
                self
            )
        );
    }
}

impl Handler<ConsensusMessage> for Network {
    type Result = ();

    fn handle(&mut self, msg: ConsensusMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(sender) = &mut self.command_sender {
            if
                let Err(e) = sender.try_send(SwarmCommand::ReceiveDataFrame {
                    peer_ids: self.peer_ids.clone(),
                    msg: vec![],
                })
            {
                eprintln!("Failed to send dial command: {:?}", e);
            }
        } else {
            eprintln!("Command sender not initialized");
        }
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

        let swarm = libp2p::SwarmBuilder
            ::with_new_identity()
            .with_tokio()
            .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)
            .expect("msg")
            .with_behaviour(|key| {
                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id()
                )?;

                let protocols = vec![("/message_protocol/1", ProtocolSupport::Full)];

                let config = Config::default();
                let request_response = request_response::Behaviour::new(protocols, config);

                Ok(Behaviour {
                    mdns,
                    request_response,
                })
            })
            .expect("msg")
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        Network {
            swarm: Some(swarm),
            peer_ids: HashSet::new(),
            command_sender: None,
            consensus_addr: None,
        }
    }
}

async fn run_swarm_loop(
    mut swarm: Swarm<Behaviour>,
    mut cmd_rx: mpsc::Receiver<SwarmCommand>,
    network_addr: Addr<Network>
) {
    loop {
        select! {


            cmd = cmd_rx.next() => {
                match cmd {
                    Some(SwarmCommand::ReceiveDataFrame {  peer_ids, msg }) => {
                        if peer_ids.is_empty() {
                            println!("No peers received.");
                        } else {
                            for peer in peer_ids {
                               let request = MessageRequest(peer.to_string());
                               swarm.behaviour_mut().request_response.send_request(&peer, request);
                            }
                        }
                    }

                    Some(SwarmCommand::Shutdown) => {
                        println!("Swarm: Shutdown");
                        break;
                    }

                    _ => {
                        println!("Received an unknown command.");
                    }
                }
            }

            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (_, multiaddr) in list {
                            if let Err(e) = swarm.dial(multiaddr.clone()) {
                                println!("Dial error: {:?}", e);
                            }
                            else {
                                println!("Successfully dialed peer: {multiaddr}");
                            }
                        }
                    },

                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        network_addr.do_send(SwarmEventMessage::ConnectionEstablished(peer_id));

                    }

                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        println!("Outgoing connection error to {:?}: {:?}", peer_id, error);
                    },

                    SwarmEvent::IncomingConnectionError { error, .. } => {
                        println!("Incoming connection error: {:?}", error);
                    },

                    SwarmEvent::Dialing { peer_id, connection_id } => {
                        println!("Dialing peer: {:?}, connection_id: {:?}", peer_id, connection_id);
                    },

                    SwarmEvent::ConnectionClosed { peer_id, endpoint, .. } => {
                        println!("Connection closed with {peer_id} via {:?}", endpoint);
                        network_addr.do_send(SwarmEventMessage::ConnectionClosed(peer_id));
                    },


                    SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                        for (peer_id, _multiaddr) in list {
                            println!("mDNS discover peer has expired: {peer_id}");
                        }
                    },

                    SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(event)) => {
                        match event {
                            request_response::Event::Message { peer, message } => {
                                match message {

                                    request_response::Message::Request { channel, request, .. } => {
                                       println!("Received request from {}: {:?}", peer, request);

                                       match network_addr.send(NetworkEventMessage::Request(peer, "hello".to_string())).await {
                                            Ok(response) => {
                                                println!("✅ Received response: {:?}", response);
                                                // Process the response data here
                                            }
                                            Err(e) => {
                                                tracing::error!("⚠️ NetworkEventMessage Error: {:?}", e);
                                            }
                                        }

                                        // if let Err(e) = swarm.behaviour_mut().request_response.send_response(
                                        //                     channel,
                                        //                     MessageResponse(request.0)
                                        //                 ) {
                                        //                     eprintln!("Failed to send response: {:?}", e);
                                        //                 }

                                    }

                                    request_response::Message::Response { request_id, response } => {
                                        println!("Received response for request {}: {:?}", request_id, response);

                                        match network_addr.send(NetworkEventMessage::Response(peer, "Received response".to_string())).await {
                                            Ok(response) => {
                                                println!("✅ Received response: {:?}", response);
                                                // Process the response data here
                                            }
                                            Err(e) => {
                                                tracing::error!("⚠️ NetworkEventMessage Error: {:?}", e);
                                            }
                                        }
                                        // match network_addr.send(NetWorkEvent(response.0)).await {
                                        //     Ok(result) => if let Ok(message) = result {
                                        //        println!("{:?}", message);
                                        //     }
                                        //     Err(_) => println!("error")
                                        // }


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
                }
            }
        }
    }
}

impl Handler<NetworkEventMessage> for Network {
    type Result = Result<String, NetworkError>;

    fn handle(&mut self, msg: NetworkEventMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            NetworkEventMessage::Request(peer_id, message) => {
                println!("message from {} for request {}", peer_id, message);
                if message.is_empty() {
                    return Err(NetworkError::InvalidData);
                }

                let response = format!("Processed request: {} and {}", message, peer_id);
                Ok(response.to_uppercase())
            }
            NetworkEventMessage::Response(peer_id, message) => {
                println!("message from {} for request {}", peer_id, message);
                if message.is_empty() {
                    return Err(NetworkError::InvalidData);
                }

                let response = format!("Processed request: {} and {}", message, peer_id);
                Ok(response.to_uppercase())
            }
        }
    }
}

impl Handler<SwarmEventMessage> for Network {
    type Result = ();
    fn handle(&mut self, msg: SwarmEventMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            SwarmEventMessage::ConnectionEstablished(peer_id) => {
                let is_new = self.peer_ids.insert(peer_id);

                if is_new {
                    println!(
                        "New connection established with {peer_id}. Total peers: {}",
                        self.peer_ids.len()
                    );
                } else {
                    println!("Peer {peer_id} was already in the set.");
                }
            }

            SwarmEventMessage::ConnectionClosed(peer_id) => {
                let was_present = self.peer_ids.remove(&peer_id);

                if was_present {
                    println!("Removed {peer_id}. Total peers: {}", self.peer_ids.len());
                } else {
                    println!("Peer {peer_id} not found in set.");
                }
            }
        }
    }
}
impl Network {}
