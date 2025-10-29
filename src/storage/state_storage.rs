use crate::raft::types::{NodeId, Term};
use crate::util::errors::{RaftError, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

/// Persistent state that must survive crashes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    /// Latest term server has seen (initialized to 0)
    pub current_term: Term,
    /// Candidate that received vote in current term (or None)
    pub voted_for: Option<NodeId>,
}

impl Default for PersistentState {
    fn default() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
        }
    }
}

/// Trait for persistent state storage
pub trait StateStorage: Send {
    fn save_term(&mut self, term: Term) -> Result<()>;
    fn load_term(&self) -> Result<Term>;
    fn save_voted_for(&mut self, peer_id: Option<NodeId>) -> Result<()>;
    fn load_voted_for(&self) -> Result<Option<NodeId>>;
    fn save_state(&mut self, state: &PersistentState) -> Result<()>;
    fn load_state(&self) -> Result<PersistentState>;
}

/// File-based state storage implementation
pub struct FileStateStorage {
    data_dir: PathBuf,
    state: PersistentState,
}

impl FileStateStorage {
    pub fn new(data_dir: PathBuf) -> Result<Self> {
        // Create data directory if it doesn't exist
        fs::create_dir_all(&data_dir)?;

        let mut storage = Self {
            data_dir,
            state: PersistentState::default(),
        };

        // Load existing state from disk
        storage.state = storage.load_from_disk()?;

        Ok(storage)
    }

    fn state_file_path(&self) -> PathBuf {
        self.data_dir.join("raft_state.bin")
    }

    fn load_from_disk(&self) -> Result<PersistentState> {
        let state_path = self.state_file_path();

        if !state_path.exists() {
            return Ok(PersistentState::default());
        }

        let mut file = File::open(&state_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        if buffer.is_empty() {
            return Ok(PersistentState::default());
        }

        let state: PersistentState = bincode::deserialize(&buffer)?;

        tracing::info!(
            "Loaded persistent state: term={}, voted_for={:?}",
            state.current_term,
            state.voted_for
        );

        Ok(state)
    }

    fn save_to_disk(&self) -> Result<()> {
        let state_path = self.state_file_path();
        let encoded = bincode::serialize(&self.state)?;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&state_path)?;

        file.write_all(&encoded)?;
        file.sync_all()?;

        Ok(())
    }
}

impl StateStorage for FileStateStorage {
    fn save_term(&mut self, term: Term) -> Result<()> {
        self.state.current_term = term;
        self.save_to_disk()
    }

    fn load_term(&self) -> Result<Term> {
        Ok(self.state.current_term)
    }

    fn save_voted_for(&mut self, peer_id: Option<NodeId>) -> Result<()> {
        self.state.voted_for = peer_id;
        self.save_to_disk()
    }

    fn load_voted_for(&self) -> Result<Option<NodeId>> {
        Ok(self.state.voted_for.clone())
    }

    fn save_state(&mut self, state: &PersistentState) -> Result<()> {
        self.state = state.clone();
        self.save_to_disk()
    }

    fn load_state(&self) -> Result<PersistentState> {
        Ok(self.state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load_term() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = FileStateStorage::new(temp_dir.path().to_path_buf()).unwrap();

        storage.save_term(5).unwrap();
        assert_eq!(storage.load_term().unwrap(), 5);
    }

    #[test]
    fn test_save_and_load_voted_for() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = FileStateStorage::new(temp_dir.path().to_path_buf()).unwrap();

        storage.save_voted_for(Some("node-1".to_string())).unwrap();
        assert_eq!(
            storage.load_voted_for().unwrap(),
            Some("node-1".to_string())
        );

        storage.save_voted_for(None).unwrap();
        assert_eq!(storage.load_voted_for().unwrap(), None);
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        {
            let mut storage = FileStateStorage::new(path.clone()).unwrap();
            storage.save_term(10).unwrap();
            storage.save_voted_for(Some("node-2".to_string())).unwrap();
        }

        // Reload from disk
        let storage = FileStateStorage::new(path).unwrap();
        assert_eq!(storage.load_term().unwrap(), 10);
        assert_eq!(
            storage.load_voted_for().unwrap(),
            Some("node-2".to_string())
        );
    }
}
