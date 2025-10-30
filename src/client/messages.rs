use crate::consensus::actor::Consensus;
use actix::prelude::*;
use serde::{Deserialize, Serialize};

/// Message to submit a transaction from the client/bot to the consensus layer
#[derive(Message, Debug, Clone)]
#[rtype(result = "Result<u64, String>")]
pub struct SubmitTransaction {
    pub bot_id: String,
    pub data: Vec<u8>,
}

/// Command to be broadcast across all bots in the cluster
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct BroadcastCommand {
    pub command: String,
    pub args: Vec<String>,
    pub from_bot: String,
}

/// Message to notify client about transaction result
#[derive(Message, Debug, Clone)]
#[rtype(result = "()")]
pub struct TransactionResult {
    pub tx_id: u64,
    pub status: TransactionStatus,
}

#[derive(Debug, Clone)]
pub enum TransactionStatus {
    Committed,
    Failed(String),
    Pending,
}

/// Message to set the consensus actor address in the client
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetConsensusAddr(pub Addr<Consensus>);

/// Message to get node status
#[derive(Message)]
#[rtype(result = "NodeStatus")]
pub struct GetNodeStatus;

#[derive(Debug, Clone)]
pub struct NodeStatus {
    pub bot_id: String,
    pub is_connected: bool,
    pub pending_transactions: usize,
    pub total_transactions: u64,
}
