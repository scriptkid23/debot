// Data Flow
// Step 1: A Response Layer receives incoming data from a node, creates a Network Event, and forwards it to the Network Layer.
// Step 2: The Network Layer processes the event and generates a Request Message, broadcasting it to other nodes.
// Step 3: The receiving node’s Request Layer captures the request and forwards it to the Response Layer.
// Step 4: The Response Layer processes the request, generates a response, and restarts the cycle.

use std::{collections::HashSet, time::Duration, vec};

use crate::{
    consensus::actor::Consensus,
    network::{
        behaviour::BehaviourEvent,
        codec::{MessageRequest, MessageResponse, NetworkMessage},
        peer_registry::PeerRegistry,
    },
    raft::actor::SendRaftMessage,
};
use actix::prelude::*;
use futures::{channel::mpsc, StreamExt};
use libp2p::{
    mdns, noise,
    request_response::{self, Config, ProtocolSupport},
    swarm::SwarmEvent,
    tcp, yamux, PeerId, Swarm,
};
use tokio::select;

use super::behaviour::Behaviour;

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetConsensusAddr(pub Addr<Consensus>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct ConsensusMessage(pub Vec<u8>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastTestMessage {
    pub chat_id: i64,
}

enum SwarmCommand {
    ReceiveDataFrame {
        peer_ids: HashSet<PeerId>,
        #[allow(dead_code)]
        msg: Vec<u8>,
    },
    SendRaftMessage {
        peer_id: PeerId,
        message: NetworkMessage,
    },
    BroadcastTest {
        chat_id: i64,
    },
    #[allow(dead_code)]
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
    peer_registry: PeerRegistry,
    swarm: Option<Swarm<Behaviour>>,
    command_sender: Option<mpsc::Sender<SwarmCommand>>,
    consensus_addr: Option<Addr<Consensus>>,
    network_addr: Option<Addr<Network>>,
}

impl Actor for Network {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("Started network listener");

        let mut swarm = self.swarm.take().expect("Swarm should be initialized");

        if let Err(e) =
            swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap_or_else(|_| {
                panic!("Invalid multiaddr provided. Check your configuration.")
            }))
        {
            tracing::error!("Failed to start listening: {:?}", e);
        } else {
            tracing::info!("Listening on all interfaces on a random port");
        }

        let (command_sender, command_receiver) = mpsc::channel(32);

        self.command_sender = Some(command_sender);
        let network_addr = ctx.address();
        self.network_addr = Some(network_addr.clone());

        // Start swarm loop WITHOUT consensus_addr initially
        // It will be provided dynamically when messages arrive
        ctx.spawn(
            (async move { run_swarm_loop(swarm, command_receiver, network_addr).await })
                .into_actor(self),
        );
    }
}

impl Handler<ConsensusMessage> for Network {
    type Result = ();

    fn handle(&mut self, _msg: ConsensusMessage, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(sender) = &mut self.command_sender {
            if let Err(e) = sender.try_send(SwarmCommand::ReceiveDataFrame {
                peer_ids: self.peer_ids.clone(),
                msg: vec![],
            }) {
                eprintln!("Failed to send dial command: {:?}", e);
            }
        } else {
            eprintln!("Command sender not initialized");
        }
    }
}

impl Handler<BroadcastTestMessage> for Network {
    type Result = ();

    fn handle(&mut self, msg: BroadcastTestMessage, _ctx: &mut Self::Context) -> Self::Result {
        tracing::info!(
            "📡 Broadcasting test message to all peers for chat_id: {}",
            msg.chat_id
        );

        if let Some(sender) = &mut self.command_sender {
            if let Err(e) = sender.try_send(SwarmCommand::BroadcastTest {
                chat_id: msg.chat_id,
            }) {
                tracing::error!("Failed to send broadcast test command: {:?}", e);
            }
        } else {
            tracing::error!("Command sender not initialized");
        }
    }
}

impl Handler<SetConsensusAddr> for Network {
    type Result = ();

    fn handle(&mut self, msg: SetConsensusAddr, _ctx: &mut Self::Context) -> Self::Result {
        self.consensus_addr = Some(msg.0);
        tracing::info!("✅ Consensus address set in Network actor");
    }
}

/// Message to get consensus address
#[derive(Message)]
#[rtype(result = "Option<Addr<Consensus>>")]
struct GetConsensusAddr;

impl Handler<GetConsensusAddr> for Network {
    type Result = Option<Addr<Consensus>>;

    fn handle(&mut self, _msg: GetConsensusAddr, _ctx: &mut Self::Context) -> Self::Result {
        self.consensus_addr.clone()
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
                // Configure mDNS with longer TTL to prevent premature peer expiry
                let mdns_config = mdns::Config {
                    ttl: std::time::Duration::from_secs(6 * 60), // 6 minutes
                    query_interval: std::time::Duration::from_secs(5 * 60), // 5 minutes
                    ..Default::default()
                };
                let mdns = mdns::tokio::Behaviour::new(mdns_config, key.public().to_peer_id())?;

                let protocols = vec![("/message_protocol/1", ProtocolSupport::Full)];

                let config = Config::default();
                let request_response = request_response::Behaviour::new(protocols, config);

                Ok(Behaviour {
                    mdns,
                    request_response,
                })
            })
            .expect("msg")
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(10 * 60))) // 10 minutes
            .build();

        Network {
            swarm: Some(swarm),
            peer_ids: HashSet::new(),
            peer_registry: PeerRegistry::new(),
            command_sender: None,
            consensus_addr: None,
            network_addr: None,
        }
    }
}

async fn run_swarm_loop(
    mut swarm: Swarm<Behaviour>,
    mut cmd_rx: mpsc::Receiver<SwarmCommand>,
    network_addr: Addr<Network>,
) {
    loop {
        select! {


            cmd = cmd_rx.next() => {
                match cmd {
                    Some(SwarmCommand::ReceiveDataFrame {  peer_ids, msg: _ }) => {
                        if peer_ids.is_empty() {
                            tracing::debug!("No peers to send to.");
                        } else {
                            for peer in peer_ids {
                               // Send heartbeat as default message
                               let request = MessageRequest(NetworkMessage::Heartbeat);
                               swarm.behaviour_mut().request_response.send_request(&peer, request);
                            }
                        }
                    }

                    Some(SwarmCommand::SendRaftMessage { peer_id, message }) => {
                        tracing::debug!("Sending Raft message to {}", peer_id);
                        let request = MessageRequest(message);
                        swarm.behaviour_mut().request_response.send_request(&peer_id, request);
                    }

                    Some(SwarmCommand::BroadcastTest { chat_id }) => {
                        tracing::info!("📡 Broadcasting test message to all peers for chat_id: {}", chat_id);
                        // Get all connected peers from the swarm
                        let connected_peers: Vec<_> = swarm.connected_peers().cloned().collect();

                        if connected_peers.is_empty() {
                            tracing::warn!("No peers to broadcast to");
                        } else {
                            for peer in connected_peers {
                                tracing::debug!("Sending test to peer: {}", peer);
                                let request = MessageRequest(NetworkMessage::Test(chat_id));
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
                        // mDNS records expired, but keep existing connections alive
                        // The peers will be re-discovered in the next mDNS query
                        for (peer_id, _multiaddr) in list {
                            tracing::debug!("mDNS peer expired (will re-discover): {peer_id}");
                            // NOTE: We intentionally do NOT close the connection here
                            // libp2p will maintain the connection until it's explicitly closed
                            // or fails. mDNS expiry just means the discovery record expired,
                            // not that the peer is actually gone.
                        }
                    },

                    SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(event)) => {
                        match event {
                            request_response::Event::Message { peer, message } => {
                                match message {

                                    request_response::Message::Request { channel, request, .. } => {
                                       tracing::debug!("Received request from {}: {:?}", peer, request);

                                       match request.0 {
                                           NetworkMessage::Raft(raft_msg) => {
                                               // Check message type for appropriate logging
                                               let is_heartbeat = matches!(
                                                   &raft_msg,
                                                   crate::raft::rpc::RaftMessage::AppendEntries(req) if req.entries.is_empty()
                                               );
                                               let is_request_vote = matches!(
                                                   &raft_msg,
                                                   crate::raft::rpc::RaftMessage::RequestVote(_)
                                               );

                                               if is_heartbeat {
                                                   tracing::debug!("Received heartbeat from {}", peer);
                                               } else if is_request_vote {
                                                   tracing::info!("📥 Network received RequestVote from {} - forwarding to Consensus", peer);
                                               } else {
                                                   tracing::info!("Received Raft message from {}: {:?}", peer, raft_msg);
                                               }

                                               // Get consensus address dynamically from network actor
                                               match network_addr.send(GetConsensusAddr).await {
                                                   Ok(Some(consensus)) => {
                                                       // Convert PeerId to NodeId
                                                       let from_node_id = format!("node-{}", peer);

                                                       // Forward to Consensus for processing
                                                       match consensus.send(crate::consensus::actor::HandleIncomingRaftMessage {
                                                           from: from_node_id,
                                                           message: raft_msg,
                                                       }).await {
                                                           Ok(Ok(response_msg)) => {
                                                               // Send Raft response back
                                                               let response = MessageResponse(NetworkMessage::Raft(response_msg));
                                                               if let Err(e) = swarm.behaviour_mut()
                                                                   .request_response
                                                                   .send_response(channel, response) {
                                                                   tracing::error!("Failed to send Raft response to {}: {:?}", peer, e);
                                                               } else if !is_heartbeat {
                                                                   tracing::info!("✅ Sent Raft response to {}", peer);
                                                               }
                                                           }
                                                           Ok(Err(e)) => {
                                                               tracing::error!("Consensus error processing Raft message: {}", e);
                                                           }
                                                           Err(e) => {
                                                               tracing::error!("Failed to forward to Consensus: {:?}", e);
                                                           }
                                                       }
                                                   }
                                                   Ok(None) => {
                                                       tracing::warn!("Consensus layer not initialized, dropping Raft message");
                                                   }
                                                   Err(e) => {
                                                       tracing::error!("Failed to get consensus address: {:?}", e);
                                                   }
                                               }
                                           }
                                           NetworkMessage::Heartbeat => {
                                               // Simple heartbeat response
                                               tracing::debug!("Received heartbeat from {}", peer);
                                               let response = MessageResponse(NetworkMessage::Heartbeat);
                                               if let Err(e) = swarm.behaviour_mut()
                                                   .request_response
                                                   .send_response(channel, response) {
                                                   tracing::error!("Failed to send heartbeat response to {}: {:?}", peer, e);
                                               }
                                           }
                                           NetworkMessage::Test(chat_id) => {
                                               tracing::info!("🔔 Received Test message from {} for chat_id: {}", peer, chat_id);

                                               // Forward to Consensus to trigger bot response
                                               match network_addr.send(GetConsensusAddr).await {
                                                   Ok(Some(consensus)) => {
                                                       consensus.do_send(crate::consensus::actor::HandleTestMessage {
                                                           chat_id,
                                                       });
                                                       tracing::info!("✅ Test message forwarded to Consensus");
                                                   }
                                                   Ok(None) => {
                                                       tracing::warn!("Consensus not initialized, dropping Test message");
                                                   }
                                                   Err(e) => {
                                                       tracing::error!("Failed to get consensus address: {:?}", e);
                                                   }
                                               }

                                               // Send ack response
                                               let response = MessageResponse(NetworkMessage::Heartbeat);
                                               if let Err(e) = swarm.behaviour_mut()
                                                   .request_response
                                                   .send_response(channel, response) {
                                                   tracing::error!("Failed to send Test response to {}: {:?}", peer, e);
                                               }
                                           }
                                       }
                                    }

                                    request_response::Message::Response { request_id: _, response } => {
                                        // Process the response based on message type
                                        match response.0 {
                                            NetworkMessage::Test(_) => {
                                                // Test responses are just acks, no need to process
                                                tracing::debug!("Received Test response from {}", peer);
                                            }
                                            NetworkMessage::Raft(raft_response) => {
                                                // Check response type for appropriate logging
                                                let is_heartbeat_response = matches!(
                                                    &raft_response,
                                                    crate::raft::rpc::RaftMessage::AppendEntriesResponse(_)
                                                );
                                                let is_vote_response = matches!(
                                                    &raft_response,
                                                    crate::raft::rpc::RaftMessage::RequestVoteResponse(_)
                                                );

                                                if is_vote_response {
                                                    tracing::info!("📬 Network received RequestVoteResponse from {} - forwarding to Consensus", peer);
                                                } else if !is_heartbeat_response {
                                                    tracing::info!("Received Raft response from {}: {:?}", peer, raft_response);
                                                }

                                                // Forward response to Consensus for processing
                                                match network_addr.send(GetConsensusAddr).await {
                                                    Ok(Some(consensus)) => {
                                                        let from_node_id = format!("node-{}", peer);

                                                        // Forward response to Consensus/Raft
                                                        match consensus.send(crate::consensus::actor::HandleIncomingRaftMessage {
                                                            from: from_node_id.clone(),
                                                            message: raft_response,
                                                        }).await {
                                                            Ok(Ok(_)) => {
                                                                tracing::debug!("Raft response processed by consensus");
                                                            }
                                                            Ok(Err(e)) => {
                                                                tracing::error!("Error processing Raft response: {}", e);
                                                            }
                                                            Err(e) => {
                                                                tracing::error!("Failed to forward Raft response: {:?}", e);
                                                            }
                                                        }
                                                    }
                                                    Ok(None) => {
                                                        tracing::warn!("Consensus not initialized, dropping Raft response");
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to get consensus address: {:?}", e);
                                                    }
                                                }
                                            }
                                            NetworkMessage::Heartbeat => {
                                                tracing::debug!("Received heartbeat response from {}", peer);
                                            }
                                        }
                                    }
                                }
                            }
                            request_response::Event::OutboundFailure { peer, request_id, error } => {
                                tracing::error!("⚠️ Outbound failure for request {} to {}: {:?}", request_id, peer, error);
                            }

                            request_response::Event::InboundFailure { peer, request_id, error } => {
                                tracing::error!("⚠️ Inbound failure from {} for request {}: {:?}", peer, request_id, error);
                            }

                            request_response::Event::ResponseSent { peer, request_id } => {
                                tracing::debug!("Response sent to {} for request {}", peer, request_id);
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

    fn handle(&mut self, msg: NetworkEventMessage, _ctx: &mut Self::Context) -> Self::Result {
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
    fn handle(&mut self, msg: SwarmEventMessage, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            SwarmEventMessage::ConnectionEstablished(peer_id) => {
                let is_new = self.peer_ids.insert(peer_id);

                if is_new {
                    tracing::info!(
                        "🔗 New connection established with {peer_id}. Total peers: {}",
                        self.peer_ids.len()
                    );

                    // Auto-register peer with NodeId
                    // For now, use peer_id as node_id (can be improved with peer exchange protocol)
                    let node_id = format!("node-{}", peer_id);
                    self.peer_registry.register(peer_id, node_id.clone());
                    tracing::info!("Registered peer {} as node {}", peer_id, node_id);

                    // Log full peer topology for diagnostics
                    let all_peers = self.peer_registry.all_node_ids();
                    tracing::info!(
                        "🌐 Current network topology - Connected peers: {:?}",
                        all_peers
                    );

                    // Update Raft peers list
                    if let Some(consensus) = &self.consensus_addr {
                        consensus.do_send(crate::consensus::actor::UpdatePeers {
                            peer_ids: all_peers,
                        });
                    }
                } else {
                    tracing::debug!("Peer {peer_id} was already in the set.");
                }
            }

            SwarmEventMessage::ConnectionClosed(peer_id) => {
                let was_present = self.peer_ids.remove(&peer_id);

                if was_present {
                    tracing::info!(
                        "🔌 Connection closed with {peer_id}. Total peers: {}",
                        self.peer_ids.len()
                    );

                    // Remove from peer registry
                    self.peer_registry.remove_peer(&peer_id);

                    // Log remaining peer topology for diagnostics
                    let remaining_peers = self.peer_registry.all_node_ids();
                    tracing::info!(
                        "🌐 Network topology after disconnect - Remaining peers: {:?}",
                        remaining_peers
                    );

                    // Update Raft peers list
                    if let Some(consensus) = &self.consensus_addr {
                        consensus.do_send(crate::consensus::actor::UpdatePeers {
                            peer_ids: remaining_peers,
                        });
                    }
                } else {
                    tracing::debug!("Peer {peer_id} not found in set.");
                }
            }
        }
    }
}

/// Handler for outgoing Raft messages
impl Handler<SendRaftMessage> for Network {
    type Result = ();

    fn handle(&mut self, msg: SendRaftMessage, _ctx: &mut Self::Context) -> Self::Result {
        // Convert NodeId to PeerId
        let peer_id = match self.peer_registry.get_peer_id(&msg.to) {
            Some(peer_id) => peer_id,
            None => {
                tracing::warn!(
                    "❌ Cannot send message to {}: peer not found in registry. Known peers: {:?}",
                    msg.to,
                    self.peer_registry.all_node_ids()
                );
                return;
            }
        };

        // Check message type for appropriate logging
        let is_request_vote = matches!(&msg.message, crate::raft::rpc::RaftMessage::RequestVote(_));

        if is_request_vote {
            tracing::info!(
                "📮 Network sending RequestVote from local node to {} (peer_id: {})",
                msg.to,
                peer_id
            );
        }

        // Wrap RaftMessage in NetworkMessage
        let network_msg = NetworkMessage::Raft(msg.message.clone());

        // Send via command channel to swarm loop
        if let Some(sender) = &mut self.command_sender {
            if let Err(e) = sender.try_send(SwarmCommand::SendRaftMessage {
                peer_id,
                message: network_msg,
            }) {
                tracing::error!("Failed to send Raft message to {}: {:?}", msg.to, e);
            } else {
                if !is_request_vote {
                    tracing::debug!("Queued Raft message to {}", msg.to);
                }
            }
        } else {
            tracing::error!("Command sender not initialized");
        }
    }
}

impl Network {}
