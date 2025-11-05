use crate::raft::types::LogEntry;
use crate::util::errors::{RaftError, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

/// Trait for persistent log storage
pub trait LogStorage: Send {
    fn append(&mut self, entries: Vec<LogEntry>) -> Result<()>;
    fn get(&self, index: u64) -> Result<Option<LogEntry>>;
    fn get_range(&self, start: u64, end: u64) -> Result<Vec<LogEntry>>;
    fn last_index(&self) -> u64;
    fn last_term(&self) -> u64;
    fn truncate(&mut self, from_index: u64) -> Result<()>;
}

/// File-based log storage implementation using bincode
pub struct FileLogStorage {
    data_dir: PathBuf,
    logs: Vec<LogEntry>,
}

impl FileLogStorage {
    pub fn new(data_dir: PathBuf) -> Result<Self> {
        // Create data directory if it doesn't exist
        fs::create_dir_all(&data_dir)?;

        let mut storage = Self {
            data_dir,
            logs: Vec::new(),
        };

        // Load existing logs from disk
        storage.load_from_disk()?;

        Ok(storage)
    }

    fn log_file_path(&self) -> PathBuf {
        self.data_dir.join("raft_logs.bin")
    }

    fn load_from_disk(&mut self) -> Result<()> {
        let log_path = self.log_file_path();

        if !log_path.exists() {
            return Ok(());
        }

        let mut file = File::open(&log_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        if buffer.is_empty() {
            return Ok(());
        }

        let config = bincode::config::standard();

        match bincode::serde::decode_from_slice::<Vec<LogEntry>, _>(&buffer, config) {
            Ok((logs, _)) => {
                self.logs = logs;
                tracing::info!("Loaded {} log entries from disk", self.logs.len());
                Ok(())
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load log entries (possibly old format): {}. Starting with empty log.",
                    e
                );
                // Delete corrupted/incompatible file
                let _ = fs::remove_file(&log_path);
                self.logs = Vec::new();
                Ok(())
            }
        }
    }

    fn save_to_disk(&self) -> Result<()> {
        let log_path = self.log_file_path();
        let config = bincode::config::standard();
        let encoded = bincode::serde::encode_to_vec(&self.logs, config)?;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&log_path)?;

        file.write_all(&encoded)?;
        file.sync_all()?;

        Ok(())
    }
}

impl LogStorage for FileLogStorage {
    fn append(&mut self, entries: Vec<LogEntry>) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        for entry in entries {
            // Validate indices are sequential
            if !self.logs.is_empty() {
                let expected_index = self.last_index() + 1;
                if entry.index != expected_index {
                    return Err(RaftError::LogInconsistency);
                }
            } else if entry.index != 1 {
                return Err(RaftError::LogInconsistency);
            }

            self.logs.push(entry);
        }

        self.save_to_disk()?;

        Ok(())
    }

    fn get(&self, index: u64) -> Result<Option<LogEntry>> {
        if index == 0 || self.logs.is_empty() {
            return Ok(None);
        }

        // Log indices are 1-based
        let array_index = (index - 1) as usize;

        if array_index >= self.logs.len() {
            return Ok(None);
        }

        Ok(Some(self.logs[array_index].clone()))
    }

    fn get_range(&self, start: u64, end: u64) -> Result<Vec<LogEntry>> {
        if start == 0 || start > end || self.logs.is_empty() {
            return Ok(Vec::new());
        }

        let start_idx = (start - 1) as usize;
        let end_idx = std::cmp::min((end - 1) as usize + 1, self.logs.len());

        if start_idx >= self.logs.len() {
            return Ok(Vec::new());
        }

        Ok(self.logs[start_idx..end_idx].to_vec())
    }

    fn last_index(&self) -> u64 {
        self.logs.last().map(|e| e.index).unwrap_or(0)
    }

    fn last_term(&self) -> u64 {
        self.logs.last().map(|e| e.term).unwrap_or(0)
    }

    fn truncate(&mut self, from_index: u64) -> Result<()> {
        if from_index == 0 {
            return Ok(());
        }

        let truncate_pos = (from_index - 1) as usize;

        if truncate_pos < self.logs.len() {
            self.logs.truncate(truncate_pos);
            self.save_to_disk()?;
            tracing::info!("Truncated log from index {}", from_index);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_append_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = FileLogStorage::new(temp_dir.path().to_path_buf()).unwrap();

        let entries = vec![
            LogEntry::new(1, 1, vec![1, 2, 3]),
            LogEntry::new(1, 2, vec![4, 5, 6]),
        ];

        storage.append(entries.clone()).unwrap();

        assert_eq!(storage.last_index(), 2);
        assert_eq!(storage.get(1).unwrap().unwrap(), entries[0]);
        assert_eq!(storage.get(2).unwrap().unwrap(), entries[1]);
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        {
            let mut storage = FileLogStorage::new(path.clone()).unwrap();
            let entries = vec![LogEntry::new(1, 1, vec![1, 2, 3])];
            storage.append(entries).unwrap();
        }

        // Reload from disk
        let storage = FileLogStorage::new(path).unwrap();
        assert_eq!(storage.last_index(), 1);
    }
}
