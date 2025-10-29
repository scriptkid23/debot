use super::types::{LogIndex, NodeId, Term};
use std::collections::HashSet;

/// The three states a Raft node can be in
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    /// Follower state - receives updates from leader
    Follower,
    /// Candidate state - requesting votes for leadership
    Candidate,
    /// Leader state - manages log replication
    Leader,
}

impl std::fmt::Display for NodeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeState::Follower => write!(f, "Follower"),
            NodeState::Candidate => write!(f, "Candidate"),
            NodeState::Leader => write!(f, "Leader"),
        }
    }
}

/// Complete state of a Raft node
#[derive(Debug, Clone)]
pub struct RaftState {
    // Persistent state on all servers
    /// Latest term server has seen (initialized to 0)
    pub current_term: Term,
    /// Candidate that received vote in current term (or None)
    pub voted_for: Option<NodeId>,

    // Volatile state on all servers
    /// Index of highest log entry known to be committed
    pub commit_index: LogIndex,
    /// Index of highest log entry applied to state machine
    pub last_applied: LogIndex,
    /// Current role of this node
    pub state: NodeState,
    /// ID of the current leader (if known)
    pub current_leader: Option<NodeId>,
    /// This node's ID
    pub node_id: NodeId,

    // Volatile state on leaders (reinitialized after election)
    /// For each server, index of the next log entry to send to that server
    pub next_index: std::collections::HashMap<NodeId, LogIndex>,
    /// For each server, index of highest log entry known to be replicated on server
    pub match_index: std::collections::HashMap<NodeId, LogIndex>,

    // Election state for candidates
    /// Set of nodes that voted for this candidate in current election
    pub votes_received: HashSet<NodeId>,
}

impl RaftState {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            commit_index: 0,
            last_applied: 0,
            state: NodeState::Follower,
            current_leader: None,
            node_id,
            next_index: std::collections::HashMap::new(),
            match_index: std::collections::HashMap::new(),
            votes_received: HashSet::new(),
        }
    }

    /// Transition to follower state
    pub fn become_follower(&mut self, term: Term, leader: Option<NodeId>) {
        tracing::info!(
            "Node {} transitioning to Follower (term: {})",
            self.node_id,
            term
        );
        self.state = NodeState::Follower;
        self.current_term = term;
        self.voted_for = None;
        self.current_leader = leader;
        self.votes_received.clear();
    }

    /// Transition to candidate state
    pub fn become_candidate(&mut self) {
        self.current_term += 1;
        self.state = NodeState::Candidate;
        self.voted_for = Some(self.node_id.clone());
        self.current_leader = None;
        self.votes_received.clear();
        self.votes_received.insert(self.node_id.clone());

        tracing::info!(
            "Node {} transitioning to Candidate (term: {})",
            self.node_id,
            self.current_term
        );
    }

    /// Transition to leader state
    pub fn become_leader(&mut self, last_log_index: LogIndex, peer_ids: Vec<NodeId>) {
        tracing::info!(
            "Node {} transitioning to Leader (term: {})",
            self.node_id,
            self.current_term
        );

        self.state = NodeState::Leader;
        self.current_leader = Some(self.node_id.clone());

        // Reinitialize leader state
        self.next_index.clear();
        self.match_index.clear();

        for peer_id in peer_ids {
            if peer_id != self.node_id {
                self.next_index.insert(peer_id.clone(), last_log_index + 1);
                self.match_index.insert(peer_id, 0);
            }
        }

        self.votes_received.clear();
    }

    /// Add a vote for this node
    pub fn add_vote(&mut self, from: NodeId) {
        self.votes_received.insert(from);
    }

    /// Check if we have received votes from a majority
    pub fn has_majority(&self, total_nodes: usize) -> bool {
        let votes = self.votes_received.len();
        let majority = (total_nodes / 2) + 1;
        votes >= majority
    }

    /// Update term if we see a higher term
    pub fn update_term(&mut self, term: Term) -> bool {
        if term > self.current_term {
            tracing::info!(
                "Node {} updating term from {} to {}",
                self.node_id,
                self.current_term,
                term
            );
            self.become_follower(term, None);
            true
        } else {
            false
        }
    }

    /// Check if we're the leader
    pub fn is_leader(&self) -> bool {
        self.state == NodeState::Leader
    }

    /// Check if we're a candidate
    pub fn is_candidate(&self) -> bool {
        self.state == NodeState::Candidate
    }

    /// Check if we're a follower
    pub fn is_follower(&self) -> bool {
        self.state == NodeState::Follower
    }
}

