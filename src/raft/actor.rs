use actix::prelude::*;
use rand::Rng;
use std::collections::HashMap;
use std::time::Duration;

use super::election::{create_request_vote, handle_request_vote, handle_request_vote_response};
use super::log::{create_append_entries, handle_append_entries, handle_append_entries_response};
use super::rpc::{RaftMessage, RequestVoteResponse};
use super::state::{NodeState, RaftState};
use super::types::{LogEntry, NodeId};
use crate::client::BroadcastCommand;
use crate::config::RaftConfig;
use crate::storage::{FileLogStorage, FileStateStorage, LogStorage, StateStorage};
use crate::util::errors::{RaftError, Result};

/// Messages that the Raft actor can handle

/// Initialize Raft with configuration
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct Initialize {
    pub config: RaftConfig,
}

/// Set peer IDs that this node knows about
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetPeers {
    pub peer_ids: Vec<NodeId>,
}

/// Handle incoming Raft RPC message
#[derive(Message)]
#[rtype(result = "Result<RaftMessage>")]
pub struct HandleRaftMessage {
    pub from: NodeId,
    pub message: RaftMessage,
}

/// Submit a command to be replicated (only leader can handle this)
#[derive(Message)]
#[rtype(result = "Result<u64>")]
pub struct SubmitCommand {
    pub data: Vec<u8>,
}

/// Get current Raft state (for debugging/monitoring)
#[derive(Message)]
#[rtype(result = "RaftStateInfo")]
pub struct GetState;

#[derive(Debug, Clone)]
pub struct RaftStateInfo {
    pub node_id: NodeId,
    pub state: NodeState,
    pub current_term: u64,
    pub current_leader: Option<NodeId>,
    pub commit_index: u64,
    pub last_log_index: u64,
}

// Implement MessageResponse for RaftStateInfo
impl<A, M> actix::dev::MessageResponse<A, M> for RaftStateInfo
where
    A: Actor,
    M: Message<Result = RaftStateInfo>,
{
    fn handle(self, _ctx: &mut A::Context, tx: Option<actix::dev::OneshotSender<M::Result>>) {
        if let Some(tx) = tx {
            let _ = tx.send(self);
        }
    }
}

/// Trigger election timeout
struct ElectionTimeout;

impl Message for ElectionTimeout {
    type Result = ();
}

/// Trigger heartbeat (for leaders)
struct HeartbeatTimeout;

impl Message for HeartbeatTimeout {
    type Result = ();
}

/// Main Raft actor
pub struct RaftActor {
    state: RaftState,
    log_storage: Box<dyn LogStorage>,
    state_storage: Box<dyn StateStorage>,
    config: Option<RaftConfig>,
    peers: Vec<NodeId>,
    election_timeout_handle: Option<SpawnHandle>,
    heartbeat_timeout_handle: Option<SpawnHandle>,
    // Address to send outgoing RPC messages (will be set by network layer)
    network_addr: Option<Recipient<SendRaftMessage>>,
    // Address to send commands to client for execution
    client_addr: Option<Recipient<BroadcastCommand>>,
    // Counter for heartbeats sent (for periodic logging)
    heartbeat_count: u64,
    // Track last sent request to each peer for handling responses
    pending_requests: HashMap<NodeId, (u64, usize)>, // (prev_log_index, sent_entries_count)
}

/// Message to send Raft RPC to network layer
#[derive(Message)]
#[rtype(result = "()")]
pub struct SendRaftMessage {
    pub to: NodeId,
    pub message: RaftMessage,
}

/// Message to broadcast Raft RPC to all peers
#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastRaftMessage {
    pub message: RaftMessage,
}

/// Set network address for sending messages
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetNetworkAddress {
    pub addr: Recipient<SendRaftMessage>,
}

impl Actor for RaftActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        tracing::info!("Raft actor started");
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        tracing::info!("Raft actor stopped");
    }
}

/// Set client address for applying committed entries
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetClientAddress {
    pub addr: Recipient<BroadcastCommand>,
}

impl RaftActor {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            state: RaftState::new(node_id),
            log_storage: Box::new(
                FileLogStorage::new(std::path::PathBuf::from("./data/logs")).unwrap(),
            ),
            state_storage: Box::new(
                FileStateStorage::new(std::path::PathBuf::from("./data/state")).unwrap(),
            ),
            config: None,
            client_addr: None,
            peers: Vec::new(),
            election_timeout_handle: None,
            heartbeat_timeout_handle: None,
            network_addr: None,
            heartbeat_count: 0,
            pending_requests: HashMap::new(),
        }
    }

    fn reset_election_timeout(&mut self, ctx: &mut Context<Self>) {
        // Cancel existing timer
        if let Some(handle) = self.election_timeout_handle.take() {
            ctx.cancel_future(handle);
        }

        // Generate random timeout
        let config = self.config.as_ref().unwrap();
        let min_ms = config.election_timeout_min_ms;
        let max_ms = config.election_timeout_max_ms;
        let timeout_ms = rand::thread_rng().gen_range(min_ms..=max_ms);

        tracing::debug!(
            "Node {} reset election timeout to {}ms",
            self.state.node_id,
            timeout_ms
        );

        // Schedule new timeout
        let handle = ctx.run_later(Duration::from_millis(timeout_ms), |act, ctx| {
            ctx.notify(ElectionTimeout);
        });

        self.election_timeout_handle = Some(handle);
    }

    fn start_heartbeat_timer(&mut self, ctx: &mut Context<Self>) {
        // Cancel existing timer
        if let Some(handle) = self.heartbeat_timeout_handle.take() {
            ctx.cancel_future(handle);
        }

        let config = self.config.as_ref().unwrap();
        let interval = Duration::from_millis(config.heartbeat_interval_ms);

        let handle = ctx.run_interval(interval, |act, ctx| {
            ctx.notify(HeartbeatTimeout);
        });

        self.heartbeat_timeout_handle = Some(handle);
    }

    fn stop_heartbeat_timer(&mut self, ctx: &mut Context<Self>) {
        if let Some(handle) = self.heartbeat_timeout_handle.take() {
            ctx.cancel_future(handle);
        }
    }

    fn start_election(&mut self, ctx: &mut Context<Self>) {
        self.state.become_candidate();

        // Save persistent state
        if let Err(e) = self.state_storage.save_term(self.state.current_term) {
            tracing::error!("Failed to save term: {:?}", e);
        }
        if let Err(e) = self
            .state_storage
            .save_voted_for(self.state.voted_for.clone())
        {
            tracing::error!("Failed to save voted_for: {:?}", e);
        }

        // Request votes from all peers
        let request = create_request_vote(&self.state, self.log_storage.as_ref());

        tracing::info!(
            "🗳️  Node {} starting election for term {} with peers: {:?}",
            self.state.node_id,
            self.state.current_term,
            self.peers
        );

        // Send RequestVote to all peers
        if let Some(network_addr) = &self.network_addr {
            let mut request_count = 0;
            for peer in &self.peers {
                if peer != &self.state.node_id {
                    tracing::info!("📤 Sending RequestVote to {}", peer);
                    network_addr.do_send(SendRaftMessage {
                        to: peer.clone(),
                        message: RaftMessage::RequestVote(request.clone()),
                    });
                    request_count += 1;
                }
            }
            if request_count == 0 {
                tracing::warn!("⚠️  No peers to send RequestVote to!");
            } else {
                tracing::info!("Sent RequestVote to {} peers", request_count);
            }
        } else {
            tracing::error!("❌ Network address not set, cannot send RequestVote!");
        }

        // Reset election timeout
        self.reset_election_timeout(ctx);
    }

    /// Apply committed entries to the state machine
    fn apply_committed_entries(&mut self) {
        while self.state.last_applied < self.state.commit_index {
            self.state.last_applied += 1;

            // Get the entry
            match self.log_storage.get(self.state.last_applied) {
                Ok(Some(entry)) => {
                    tracing::info!(
                        "📝 Node {} applying entry {} (term {})",
                        self.state.node_id,
                        entry.index,
                        entry.term
                    );

                    // Try to deserialize as BroadcastCommand
                    match bincode::deserialize::<BroadcastCommand>(&entry.data) {
                        Ok(command) => {
                            tracing::info!(
                                "🎯 Executing broadcast command: {} from {}",
                                command.command,
                                command.from_bot
                            );

                            // Send to client for execution
                            if let Some(client_addr) = &self.client_addr {
                                client_addr.do_send(command);
                            } else {
                                tracing::warn!(
                                    "⚠️  Client address not set, cannot execute command"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                "Entry {} is not a BroadcastCommand ({}), skipping",
                                entry.index,
                                e
                            );
                        }
                    }
                }
                Ok(None) => {
                    tracing::error!(
                        "Entry {} not found in log but should be applied!",
                        self.state.last_applied
                    );
                    break;
                }
                Err(e) => {
                    tracing::error!("Failed to get entry {}: {:?}", self.state.last_applied, e);
                    break;
                }
            }
        }
    }

    fn send_heartbeats(&mut self) {
        if !self.state.is_leader() {
            return;
        }

        if let Some(network_addr) = &self.network_addr {
            self.heartbeat_count += 1;

            // Log status every 100 heartbeats (~5 seconds at 50ms interval)
            if self.heartbeat_count % 100 == 0 {
                tracing::info!(
                    "💓 Leader {} healthy - {} peers, term {}, commit_index {}",
                    self.state.node_id,
                    self.peers.len(),
                    self.state.current_term,
                    self.state.commit_index
                );
            }

            for peer in &self.peers {
                if peer != &self.state.node_id {
                    match create_append_entries(&self.state, self.log_storage.as_ref(), peer) {
                        Ok(request) => {
                            // Only log if there are actual entries (not just heartbeat)
                            if !request.entries.is_empty() {
                                tracing::info!(
                                    "Sending {} log entries to {}",
                                    request.entries.len(),
                                    peer
                                );
                            }

                            network_addr.do_send(SendRaftMessage {
                                to: peer.clone(),
                                message: RaftMessage::AppendEntries(request),
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to create AppendEntries for {}: {:?}", peer, e);
                        }
                    }
                }
            }
        }
    }
}

// Handler implementations

impl Handler<Initialize> for RaftActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Initialize, ctx: &mut Context<Self>) -> Self::Result {
        tracing::info!("Initializing Raft with config: {:?}", msg.config);

        // Validate config
        msg.config
            .validate()
            .map_err(|e| RaftError::InvalidConfig(e))?;

        // ✅ UPDATE NODE ID from config!
        self.state.node_id = msg.config.node_id.clone();
        tracing::info!("Set Raft node_id to: {}", self.state.node_id);

        // Initialize storage with configured data directory
        self.log_storage = Box::new(FileLogStorage::new(msg.config.data_dir.join("logs"))?);
        self.state_storage = Box::new(FileStateStorage::new(msg.config.data_dir.join("state"))?);

        // Load persistent state
        let persistent_state = self.state_storage.load_state()?;
        self.state.current_term = persistent_state.current_term;
        self.state.voted_for = persistent_state.voted_for;

        self.config = Some(msg.config);

        // Start as follower with election timeout
        self.reset_election_timeout(ctx);

        Ok(())
    }
}

impl Handler<SetPeers> for RaftActor {
    type Result = ();

    fn handle(&mut self, msg: SetPeers, _ctx: &mut Context<Self>) -> Self::Result {
        tracing::info!(
            "🔄 Node {} updating peer list: {:?} (total: {})",
            self.state.node_id,
            msg.peer_ids,
            msg.peer_ids.len()
        );
        self.peers = msg.peer_ids;
    }
}

impl Handler<SetNetworkAddress> for RaftActor {
    type Result = ();

    fn handle(&mut self, msg: SetNetworkAddress, _ctx: &mut Context<Self>) -> Self::Result {
        self.network_addr = Some(msg.addr);
    }
}

impl Handler<SetClientAddress> for RaftActor {
    type Result = ();

    fn handle(&mut self, msg: SetClientAddress, _ctx: &mut Context<Self>) -> Self::Result {
        self.client_addr = Some(msg.addr);
        tracing::info!("✅ Client address set for command execution");
    }
}

impl Handler<HandleRaftMessage> for RaftActor {
    type Result = Result<RaftMessage>;

    fn handle(&mut self, msg: HandleRaftMessage, ctx: &mut Context<Self>) -> Self::Result {
        match msg.message {
            RaftMessage::RequestVote(request) => {
                let response = handle_request_vote(
                    &mut self.state,
                    self.state_storage.as_mut(),
                    self.log_storage.as_ref(),
                    request,
                )?;

                // Reset election timeout if we granted the vote
                if response.vote_granted {
                    self.reset_election_timeout(ctx);
                }

                Ok(RaftMessage::RequestVoteResponse(response))
            }

            RaftMessage::RequestVoteResponse(response) => {
                // Total nodes = peers + self
                let total_nodes = self.peers.len() + 1;
                let won_election =
                    handle_request_vote_response(&mut self.state, msg.from, response, total_nodes)?;

                if won_election {
                    // We won the election, become leader
                    let last_log_index = self.log_storage.last_index();
                    self.state.become_leader(last_log_index, self.peers.clone());

                    // Stop election timeout, start heartbeat
                    if let Some(handle) = self.election_timeout_handle.take() {
                        ctx.cancel_future(handle);
                    }
                    self.start_heartbeat_timer(ctx);

                    // Send initial heartbeats
                    self.send_heartbeats();
                }

                Ok(RaftMessage::RequestVoteResponse(RequestVoteResponse {
                    term: self.state.current_term,
                    vote_granted: false,
                }))
            }

            RaftMessage::AppendEntries(request) => {
                // Reset election timeout when receiving from leader
                self.reset_election_timeout(ctx);

                let response = handle_append_entries(
                    &mut self.state,
                    self.state_storage.as_mut(),
                    self.log_storage.as_mut(),
                    request,
                )?;

                // Apply committed entries after handling AppendEntries
                self.apply_committed_entries();

                Ok(RaftMessage::AppendEntriesResponse(response))
            }

            RaftMessage::AppendEntriesResponse(response) => {
                // Get tracked request info
                let (prev_log_index, sent_count) = self
                    .pending_requests
                    .get(&msg.from)
                    .copied()
                    .unwrap_or((0, 0));

                tracing::debug!(
                    "Leader {} received AppendEntriesResponse from {}: success={}, sent_count={}, prev_index={}",
                    self.state.node_id,
                    msg.from,
                    response.success,
                    sent_count,
                    prev_log_index
                );

                let old_commit = self.state.commit_index;

                handle_append_entries_response(
                    &mut self.state,
                    self.log_storage.as_ref(),
                    msg.from.clone(),
                    response.clone(),
                    sent_count,
                    prev_log_index,
                )?;

                // Log if commit_index advanced
                if self.state.commit_index > old_commit {
                    tracing::info!(
                        "🎉 Leader {} commit_index advanced from {} to {}",
                        self.state.node_id,
                        old_commit,
                        self.state.commit_index
                    );
                }

                // Apply committed entries after handling response (leader may have advanced commit_index)
                self.apply_committed_entries();

                Ok(RaftMessage::AppendEntriesResponse(response))
            }
        }
    }
}

impl Handler<ElectionTimeout> for RaftActor {
    type Result = ();

    fn handle(&mut self, _msg: ElectionTimeout, ctx: &mut Context<Self>) -> Self::Result {
        if self.state.is_leader() {
            // Leaders don't start elections
            return;
        }

        tracing::info!(
            "Node {} election timeout, starting election",
            self.state.node_id
        );
        self.start_election(ctx);
    }
}

impl Handler<HeartbeatTimeout> for RaftActor {
    type Result = ();

    fn handle(&mut self, _msg: HeartbeatTimeout, _ctx: &mut Context<Self>) -> Self::Result {
        if !self.state.is_leader() {
            return;
        }

        // Heartbeat logging moved to send_heartbeats() - only logs when sending actual entries
        self.send_heartbeats();
    }
}

impl Handler<SubmitCommand> for RaftActor {
    type Result = Result<u64>;

    fn handle(&mut self, msg: SubmitCommand, _ctx: &mut Context<Self>) -> Self::Result {
        if !self.state.is_leader() {
            return Err(RaftError::InvalidState(
                "Only leader can accept commands".to_string(),
            ));
        }

        let next_index = self.log_storage.last_index() + 1;
        let entry = LogEntry::new(self.state.current_term, next_index, msg.data);

        self.log_storage.append(vec![entry])?;

        tracing::info!(
            "Leader {} appended entry at index {}",
            self.state.node_id,
            next_index
        );

        // If this is a single-node cluster, commit immediately
        if self.peers.is_empty() {
            self.state.commit_index = next_index;
            tracing::info!(
                "Single-node cluster: Leader {} committed entry {} immediately",
                self.state.node_id,
                next_index
            );
            // Apply the entry
            self.apply_committed_entries();
        }

        // Trigger immediate replication
        self.send_heartbeats();

        Ok(next_index)
    }
}

impl Handler<GetState> for RaftActor {
    type Result = RaftStateInfo;

    fn handle(&mut self, _msg: GetState, _ctx: &mut Context<Self>) -> Self::Result {
        RaftStateInfo {
            node_id: self.state.node_id.clone(),
            state: self.state.state,
            current_term: self.state.current_term,
            current_leader: self.state.current_leader.clone(),
            commit_index: self.state.commit_index,
            last_log_index: self.log_storage.last_index(),
        }
    }
}
