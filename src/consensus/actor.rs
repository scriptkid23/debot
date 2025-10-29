use actix::prelude::*;

use crate::config::RaftConfig;
use crate::network::actor::Network;
use crate::raft::actor::{
    GetState, HandleRaftMessage, Initialize, RaftActor, RaftStateInfo, SendRaftMessage,
    SetNetworkAddress, SetPeers, SubmitCommand,
};
use crate::raft::rpc::RaftMessage;
use crate::raft::types::NodeId;

pub struct Consensus {
    network_addr: Option<Addr<Network>>,
    raft_addr: Option<Addr<RaftActor>>,
}

/// Message to set the network address
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetNetworkAddr(pub Addr<Network>);

/// Message to initialize consensus with Raft config
#[derive(Message)]
#[rtype(result = "()")]
pub struct InitializeConsensus {
    pub config: RaftConfig,
}

/// Message to update peer list
#[derive(Message)]
#[rtype(result = "()")]
pub struct UpdatePeers {
    pub peer_ids: Vec<NodeId>,
}

/// Message to propose a command to the cluster
#[derive(Message)]
#[rtype(result = "Result<u64, String>")]
pub struct ProposeCommand {
    pub data: Vec<u8>,
}

/// Message to get consensus state
#[derive(Message)]
#[rtype(result = "Option<RaftStateInfo>")]
pub struct GetConsensusState;

/// Message to handle incoming Raft RPC from network
#[derive(Message)]
#[rtype(result = "Result<RaftMessage, String>")]
pub struct HandleIncomingRaftMessage {
    pub from: NodeId,
    pub message: RaftMessage,
}

#[derive(Debug)]
pub enum Network2ConsensusRequestError {
    InvalidData,
    // Add other error variants as needed
}

#[derive(Message)]
#[rtype(result = "Result<String, Network2ConsensusRequestError>")]
pub struct Network2ConsensusRequest(pub String);

impl Actor for Consensus {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        tracing::info!("Starting consensus layer");

        // Start Raft actor
        let raft = RaftActor::new("node-0".to_string()).start();
        self.raft_addr = Some(raft);
    }
}

impl Default for Consensus {
    fn default() -> Self {
        Consensus {
            network_addr: None,
            raft_addr: None,
        }
    }
}

impl Handler<SetNetworkAddr> for Consensus {
    type Result = ();

    fn handle(&mut self, msg: SetNetworkAddr, _ctx: &mut Self::Context) -> Self::Result {
        self.network_addr = Some(msg.0.clone());

        // Wire up network to raft NOW (when network address is available)
        if let Some(raft) = &self.raft_addr {
            let network_recipient: Recipient<SendRaftMessage> = msg.0.recipient();
            raft.do_send(SetNetworkAddress {
                addr: network_recipient,
            });
            tracing::info!("✅ Wired up Network ↔ Raft connection");
        } else {
            tracing::warn!("⚠️ Raft actor not started yet, cannot wire up network");
        }
    }
}

impl Handler<InitializeConsensus> for Consensus {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: InitializeConsensus, _ctx: &mut Self::Context) -> Self::Result {
        let raft_addr = self.raft_addr.clone();
        let node_id = msg.config.node_id.clone();

        Box::pin(async move {
            if let Some(raft) = raft_addr {
                tracing::info!("Initializing Raft with node_id: {}", node_id);
                match raft.send(Initialize { config: msg.config }).await {
                    Ok(Ok(())) => {
                        tracing::info!("Raft initialized successfully for node {}", node_id);
                    }
                    Ok(Err(e)) => {
                        tracing::error!("Failed to initialize Raft: {:?}", e);
                    }
                    Err(e) => {
                        tracing::error!("Failed to send initialize message to Raft: {:?}", e);
                    }
                }
            }
        })
    }
}

impl Handler<UpdatePeers> for Consensus {
    type Result = ();

    fn handle(&mut self, msg: UpdatePeers, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(raft) = &self.raft_addr {
            raft.do_send(SetPeers {
                peer_ids: msg.peer_ids,
            });
        }
    }
}

impl Handler<ProposeCommand> for Consensus {
    type Result = ResponseFuture<Result<u64, String>>;

    fn handle(&mut self, msg: ProposeCommand, _ctx: &mut Self::Context) -> Self::Result {
        let raft_addr = self.raft_addr.clone();

        Box::pin(async move {
            if let Some(raft) = raft_addr {
                match raft.send(SubmitCommand { data: msg.data }).await {
                    Ok(Ok(index)) => Ok(index),
                    Ok(Err(e)) => Err(format!("Raft error: {:?}", e)),
                    Err(e) => Err(format!("Mailbox error: {:?}", e)),
                }
            } else {
                Err("Raft not initialized".to_string())
            }
        })
    }
}

impl Handler<GetConsensusState> for Consensus {
    type Result = ResponseFuture<Option<RaftStateInfo>>;

    fn handle(&mut self, _msg: GetConsensusState, _ctx: &mut Self::Context) -> Self::Result {
        let raft_addr = self.raft_addr.clone();

        Box::pin(async move {
            if let Some(raft) = raft_addr {
                match raft.send(GetState).await {
                    Ok(state) => Some(state),
                    Err(_) => None,
                }
            } else {
                None
            }
        })
    }
}

impl Handler<Network2ConsensusRequest> for Consensus {
    type Result = Result<String, Network2ConsensusRequestError>;

    fn handle(&mut self, msg: Network2ConsensusRequest, _ctx: &mut Self::Context) -> Self::Result {
        tracing::debug!("Received data from network: {}", msg.0);
        Ok("Processed network event".to_string())
    }
}

impl Handler<HandleIncomingRaftMessage> for Consensus {
    type Result = ResponseFuture<Result<RaftMessage, String>>;

    fn handle(&mut self, msg: HandleIncomingRaftMessage, _ctx: &mut Self::Context) -> Self::Result {
        let raft_addr = self.raft_addr.clone();

        Box::pin(async move {
            if let Some(raft) = raft_addr {
                match raft
                    .send(HandleRaftMessage {
                        from: msg.from.clone(),
                        message: msg.message,
                    })
                    .await
                {
                    Ok(Ok(response)) => {
                        tracing::debug!("Raft processed message from {}", msg.from);
                        Ok(response)
                    }
                    Ok(Err(e)) => {
                        tracing::error!("Raft error: {:?}", e);
                        Err(format!("Raft error: {:?}", e))
                    }
                    Err(e) => {
                        tracing::error!("Mailbox error: {:?}", e);
                        Err(format!("Mailbox error: {:?}", e))
                    }
                }
            } else {
                Err("Raft not initialized".to_string())
            }
        })
    }
}
