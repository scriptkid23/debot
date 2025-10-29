use serde::{Deserialize, Serialize};

/// A single entry in the replicated log
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogEntry {
    /// The term when this entry was created
    pub term: u64,
    /// The index of this entry in the log
    pub index: u64,
    /// The data payload
    pub data: Vec<u8>,
}

impl LogEntry {
    pub fn new(term: u64, index: u64, data: Vec<u8>) -> Self {
        Self { term, index, data }
    }
}

/// Type alias for term numbers
pub type Term = u64;

/// Type alias for log indices
pub type LogIndex = u64;

/// Node identifier
pub type NodeId = String;

