use std::{collections::HashSet, time::Duration};

use crate::{
    consensus::actor::Consensus,
    network::{
        behaviour::BehaviourEvent,
        codec::{MessageRequest, MessageResponse},
    },
};
use actix::prelude::*;
use futures::{channel::mpsc, StreamExt};
use libp2p::{
    mdns, noise,
    request_response::{self, Config, ProtocolSupport},
    swarm::SwarmEvent,
    tcp, yamux, Multiaddr, PeerId, Swarm,
};
use tokio::select;
use tokio_util::codec::{FramedRead, LinesCodec};

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
pub struct DialPeer(pub Multiaddr);

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetConsensusAddr(pub Addr<Consensus>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct NetworkMessage(pub String);

enum SwarmCommand {
    SendMessage { data: String },
    Shutdown,
    Dial(Multiaddr),
}

/// Our Actor that manages the libp2p swarm.
pub struct Network {
    swarm: Option<Swarm<Behaviour>>,
    command_sender: Option<mpsc::Sender<SwarmCommand>>,
    consensus_addr: Option<Addr<Consensus>>,
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

        let (command_sender, command_receiver) = mpsc::channel(32);

        self.command_sender = Some(command_sender);

        let addr = ctx.address();

        ctx.spawn(
            (async move { run_swarm_loop(swarm, command_receiver, addr).await }).into_actor(self),
        );
    }
}

impl Handler<SendMessage> for Network {
    type Result = ();
    fn handle(&mut self, msg: SendMessage, ctx: &mut Self::Context) -> Self::Result {
        println!("Received SendMessage: {:?}", msg.0);
    }
}

impl Handler<NetworkMessage> for Network {
    type Result = ();

    fn handle(&mut self, msg: NetworkMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(sender) = &mut self.command_sender {
            if let Err(e) = sender.try_send(SwarmCommand::SendMessage { data: msg.0 }) {
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

        let swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .expect("msg")
            .with_behaviour(|key| {
                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id(),
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
            command_sender: None,
            consensus_addr: None,
        }
    }
}

async fn run_swarm_loop(
    mut swarm: Swarm<Behaviour>,
    mut cmd_rx: mpsc::Receiver<SwarmCommand>,
    actor_addr: Addr<Network>,
) {
    println!("swarm loop");

    let stdin = tokio::io::stdin();
    let mut stdin = FramedRead::new(stdin, LinesCodec::new());

    println!("Type 'send <message>' to send a message to peers:");

    loop {
        select! {
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
                               //TODO: send to cmd
                               let request = MessageRequest(message.to_string());
                               swarm.behaviour_mut().request_response.send_request(&peer, request);
                            }
                        }
                    }
                }
                Some(Err(e)) => eprintln!("Error reading from stdin: {:?}", e),
                None => break,
            },

            cmd = cmd_rx.next() => {
                match cmd {
                    Some(SwarmCommand::Dial(addr)) => {
                        if let Err(e) = swarm.dial(addr.clone()) {
                            println!("Dial error: {:?}", e);
                        }
                        else {
                            println!("Successfully dialed peer: {addr}");
                        }
                    }
                    Some(SwarmCommand::SendMessage {  data }) => {
                        println!("Swarm: SendMessage: {data}");

                        let peers: HashSet<PeerId> = swarm.behaviour().mdns.discovered_nodes().cloned().collect();

                        println!("{:?}", peers);

                        if peers.is_empty() {
                                print!("Nothing!")
                        }else {
                            for peer in peers {
                               //TODO: send to cmd
                               let request = MessageRequest(data.to_string());
                               swarm.behaviour_mut().request_response.send_request(&peer, request);
                            }
                        }

                        //TODO: send to swarm
                    }
                    Some(SwarmCommand::Shutdown) => {
                        println!("Swarm: Shutdown");
                        break;
                    }
                    _ => ()
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
                        println!("Connection established with {peer_id}");
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
                }
            }
        }
    }
}

impl Handler<NetworkEvent> for Network {
    type Result = ();

    fn handle(&mut self, evt: NetworkEvent, _ctx: &mut Self::Context) -> Self::Result {
        match evt {
            NetworkEvent::DiscoveredPeer(pid) => {
                println!("Actor: discovered peer = {pid}");
            }
            _ => (),
        }
    }
}

impl Handler<DialPeer> for Network {
    type Result = ();

    fn handle(&mut self, msg: DialPeer, ctx: &mut Self::Context) -> Self::Result {
        if let Some(sender) = &mut self.command_sender {
            if let Err(e) = sender.try_send(SwarmCommand::Dial(msg.0)) {
                eprintln!("Failed to send dial command: {:?}", e);
            }
        } else {
            eprintln!("Command sender not initialized");
        }
    }
}

impl Network {}
