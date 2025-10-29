use crate::raft::types::NodeId;
use libp2p::PeerId;
use std::collections::HashMap;

/// Maps between libp2p PeerId and Raft NodeId
pub struct PeerRegistry {
    peer_to_node: HashMap<PeerId, NodeId>,
    node_to_peer: HashMap<NodeId, PeerId>,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self {
            peer_to_node: HashMap::new(),
            node_to_peer: HashMap::new(),
        }
    }

    /// Register a new peer mapping
    pub fn register(&mut self, peer_id: PeerId, node_id: NodeId) {
        self.peer_to_node.insert(peer_id, node_id.clone());
        self.node_to_peer.insert(node_id, peer_id);
    }

    /// Get NodeId from PeerId
    pub fn get_node_id(&self, peer_id: &PeerId) -> Option<NodeId> {
        self.peer_to_node.get(peer_id).cloned()
    }

    /// Get PeerId from NodeId
    pub fn get_peer_id(&self, node_id: &NodeId) -> Option<PeerId> {
        self.node_to_peer.get(node_id).copied()
    }

    /// Remove a peer from registry
    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        if let Some(node_id) = self.peer_to_node.remove(peer_id) {
            self.node_to_peer.remove(&node_id);
        }
    }

    /// Get all registered node IDs
    pub fn all_node_ids(&self) -> Vec<NodeId> {
        self.node_to_peer.keys().cloned().collect()
    }

    /// Check if a peer is registered
    pub fn contains_peer(&self, peer_id: &PeerId) -> bool {
        self.peer_to_node.contains_key(peer_id)
    }

    /// Check if a node is registered
    pub fn contains_node(&self, node_id: &NodeId) -> bool {
        self.node_to_peer.contains_key(node_id)
    }
}

impl Default for PeerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
