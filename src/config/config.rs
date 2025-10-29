use std::path::PathBuf;
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftConfig {
    /// Unique identifier for this node
    pub node_id: String,
    
    /// Minimum election timeout in milliseconds (e.g., 150)
    pub election_timeout_min_ms: u64,
    
    /// Maximum election timeout in milliseconds (e.g., 300)
    pub election_timeout_max_ms: u64,
    
    /// Heartbeat interval in milliseconds (e.g., 50)
    /// Should be much less than election timeout
    pub heartbeat_interval_ms: u64,
    
    /// Directory for persistent storage
    pub data_dir: PathBuf,
}

impl RaftConfig {
    pub fn election_timeout_min(&self) -> Duration {
        Duration::from_millis(self.election_timeout_min_ms)
    }

    pub fn election_timeout_max(&self) -> Duration {
        Duration::from_millis(self.election_timeout_max_ms)
    }

    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.heartbeat_interval_ms)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.election_timeout_min_ms >= self.election_timeout_max_ms {
            return Err("election_timeout_min must be less than election_timeout_max".to_string());
        }

        if self.heartbeat_interval_ms >= self.election_timeout_min_ms {
            return Err("heartbeat_interval must be less than election_timeout_min".to_string());
        }

        if self.node_id.is_empty() {
            return Err("node_id cannot be empty".to_string());
        }

        Ok(())
    }
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            node_id: "node-0".to_string(),
            election_timeout_min_ms: 150,
            election_timeout_max_ms: 300,
            heartbeat_interval_ms: 50,
            data_dir: PathBuf::from("./data"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub listen_addr: String,
    pub protocol_version: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/0".to_string(),
            protocol_version: "/message_protocol/1".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub network: NetworkConfig,
    pub raft: RaftConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            network: NetworkConfig::default(),
            raft: RaftConfig::default(),
        }
    }
}

