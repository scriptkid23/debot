use super::rpc::{AppendEntriesRequest, AppendEntriesResponse};
use super::state::RaftState;
use super::types::{LogEntry, NodeId};
use crate::storage::{LogStorage, StateStorage};
use crate::util::errors::{RaftError, Result};

/// Handle incoming AppendEntries RPC
pub fn handle_append_entries<L: LogStorage + ?Sized, S: StateStorage + ?Sized>(
    raft_state: &mut RaftState,
    state_storage: &mut S,
    log_storage: &mut L,
    request: AppendEntriesRequest,
) -> Result<AppendEntriesResponse> {
    // If request term is greater, update our term and become follower
    if request.term > raft_state.current_term {
        raft_state.update_term(request.term);
        state_storage.save_term(raft_state.current_term)?;
    }

    // Reply false if term < currentTerm
    if request.term < raft_state.current_term {
        tracing::debug!(
            "Node {} rejected AppendEntries from {} - stale term ({} < {})",
            raft_state.node_id,
            request.leader_id,
            request.term,
            raft_state.current_term
        );

        return Ok(AppendEntriesResponse {
            term: raft_state.current_term,
            success: false,
            conflict_index: None,
            conflict_term: None,
        });
    }

    // Valid leader for this term, update leader and reset election timeout
    raft_state.current_leader = Some(request.leader_id.clone());

    // If we're a candidate and receive valid AppendEntries, become follower
    if raft_state.is_candidate() {
        raft_state.become_follower(request.term, Some(request.leader_id.clone()));
    }

    // Check log consistency
    if request.prev_log_index > 0 {
        let prev_log_entry = log_storage.get(request.prev_log_index)?;

        match prev_log_entry {
            None => {
                // We don't have the entry at prev_log_index
                tracing::debug!(
                    "Node {} rejected AppendEntries - missing entry at index {}",
                    raft_state.node_id,
                    request.prev_log_index
                );

                return Ok(AppendEntriesResponse {
                    term: raft_state.current_term,
                    success: false,
                    conflict_index: Some(log_storage.last_index() + 1),
                    conflict_term: None,
                });
            }
            Some(entry) => {
                // Check if the term matches
                if entry.term != request.prev_log_term {
                    tracing::debug!(
                        "Node {} rejected AppendEntries - term mismatch at index {} ({} != {})",
                        raft_state.node_id,
                        request.prev_log_index,
                        entry.term,
                        request.prev_log_term
                    );

                    // Find the first index with the conflicting term
                    let mut conflict_index = request.prev_log_index;
                    while conflict_index > 1 {
                        if let Ok(Some(e)) = log_storage.get(conflict_index - 1) {
                            if e.term != entry.term {
                                break;
                            }
                            conflict_index -= 1;
                        } else {
                            break;
                        }
                    }

                    return Ok(AppendEntriesResponse {
                        term: raft_state.current_term,
                        success: false,
                        conflict_index: Some(conflict_index),
                        conflict_term: Some(entry.term),
                    });
                }
            }
        }
    }

    // Append new entries
    if !request.entries.is_empty() {
        // Check for conflicts and truncate if necessary
        for (i, new_entry) in request.entries.iter().enumerate() {
            if let Ok(Some(existing_entry)) = log_storage.get(new_entry.index) {
                if existing_entry.term != new_entry.term {
                    // Conflict found, delete this entry and all that follow
                    tracing::info!(
                        "Node {} found log conflict at index {}, truncating",
                        raft_state.node_id,
                        new_entry.index
                    );
                    log_storage.truncate(new_entry.index)?;

                    // Append remaining entries
                    let remaining = request.entries[i..].to_vec();
                    log_storage.append(remaining)?;
                    break;
                }
                // Entry matches, continue checking
            } else {
                // No existing entry, append remaining entries
                let remaining = request.entries[i..].to_vec();
                log_storage.append(remaining)?;
                break;
            }
        }

        tracing::debug!(
            "Node {} appended {} entries from leader {}",
            raft_state.node_id,
            request.entries.len(),
            request.leader_id
        );
    }

    // Update commit index
    if request.leader_commit > raft_state.commit_index {
        let last_new_entry_index = log_storage.last_index();
        raft_state.commit_index = std::cmp::min(request.leader_commit, last_new_entry_index);

        tracing::debug!(
            "Node {} updated commit_index to {}",
            raft_state.node_id,
            raft_state.commit_index
        );
    }

    Ok(AppendEntriesResponse {
        term: raft_state.current_term,
        success: true,
        conflict_index: None,
        conflict_term: None,
    })
}

/// Handle AppendEntries response (for leaders)
pub fn handle_append_entries_response<L: LogStorage + ?Sized>(
    raft_state: &mut RaftState,
    log_storage: &L,
    from: NodeId,
    response: AppendEntriesResponse,
    sent_entries_count: usize,
    prev_log_index: u64,
) -> Result<()> {
    // If response term is greater, update our term and step down
    if response.term > raft_state.current_term {
        raft_state.update_term(response.term);
        return Ok(());
    }

    // Ignore if we're not leader anymore
    if !raft_state.is_leader() {
        return Ok(());
    }

    // Ignore stale responses
    if response.term < raft_state.current_term {
        return Ok(());
    }

    if response.success {
        // Update next_index and match_index
        let new_match_index = prev_log_index + sent_entries_count as u64;

        if let Some(match_idx) = raft_state.match_index.get_mut(&from) {
            *match_idx = std::cmp::max(*match_idx, new_match_index);
        }

        if let Some(next_idx) = raft_state.next_index.get_mut(&from) {
            *next_idx = new_match_index + 1;
        }

        tracing::debug!(
            "Node {} updated match_index for {} to {}",
            raft_state.node_id,
            from,
            new_match_index
        );

        // Try to advance commit_index
        advance_commit_index(raft_state, log_storage)?;
    } else {
        // AppendEntries failed, decrement next_index and retry
        if let Some(conflict_index) = response.conflict_index {
            // Use conflict information for faster backtracking
            if let Some(next_idx) = raft_state.next_index.get_mut(&from) {
                *next_idx = conflict_index;
                tracing::debug!(
                    "Node {} decremented next_index for {} to {} (conflict)",
                    raft_state.node_id,
                    from,
                    conflict_index
                );
            }
        } else if let Some(next_idx) = raft_state.next_index.get_mut(&from) {
            // Fallback: decrement by 1
            if *next_idx > 1 {
                *next_idx -= 1;
            }
            tracing::debug!(
                "Node {} decremented next_index for {} to {}",
                raft_state.node_id,
                from,
                *next_idx
            );
        }
    }

    Ok(())
}

/// Try to advance commit index based on match_index of followers
fn advance_commit_index<L: LogStorage + ?Sized>(
    raft_state: &mut RaftState,
    log_storage: &L,
) -> Result<()> {
    if !raft_state.is_leader() {
        return Ok(());
    }

    let last_log_index = log_storage.last_index();

    // Try each index from commit_index + 1 to last_log_index
    for n in (raft_state.commit_index + 1)..=last_log_index {
        // Count how many nodes have replicated this entry
        let mut count = 1; // Count ourselves

        for match_idx in raft_state.match_index.values() {
            if *match_idx >= n {
                count += 1;
            }
        }

        let total_nodes = raft_state.match_index.len() + 1; // +1 for ourselves
        let majority = (total_nodes / 2) + 1;

        if count >= majority {
            // Check that the entry is from our current term
            if let Ok(Some(entry)) = log_storage.get(n) {
                if entry.term == raft_state.current_term {
                    raft_state.commit_index = n;
                    tracing::info!(
                        "Leader {} advanced commit_index to {}",
                        raft_state.node_id,
                        n
                    );
                }
            }
        }
    }

    Ok(())
}

/// Create AppendEntries request for a specific follower
pub fn create_append_entries<L: LogStorage + ?Sized>(
    raft_state: &RaftState,
    log_storage: &L,
    follower_id: &NodeId,
) -> Result<AppendEntriesRequest> {
    let next_index = raft_state.next_index.get(follower_id).copied().unwrap_or(1);

    let prev_log_index = if next_index > 1 { next_index - 1 } else { 0 };

    let prev_log_term = if prev_log_index > 0 {
        log_storage
            .get(prev_log_index)?
            .map(|e| e.term)
            .unwrap_or(0)
    } else {
        0
    };

    let last_log_index = log_storage.last_index();

    // Get entries to send
    let entries = if next_index <= last_log_index {
        log_storage.get_range(next_index, last_log_index)?
    } else {
        Vec::new()
    };

    Ok(AppendEntriesRequest {
        term: raft_state.current_term,
        leader_id: raft_state.node_id.clone(),
        prev_log_index,
        prev_log_term,
        entries,
        leader_commit: raft_state.commit_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{FileLogStorage, FileStateStorage};
    use tempfile::TempDir;

    #[test]
    fn test_append_entries_success() {
        let temp_dir = TempDir::new().unwrap();
        let mut raft_state = RaftState::new("node-1".to_string());
        raft_state.current_term = 1;

        let mut state_storage = FileStateStorage::new(temp_dir.path().join("state")).unwrap();
        let mut log_storage = FileLogStorage::new(temp_dir.path().join("logs")).unwrap();

        let request = AppendEntriesRequest {
            term: 1,
            leader_id: "node-2".to_string(),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![LogEntry::new(1, 1, vec![1, 2, 3])],
            leader_commit: 0,
        };

        let response = handle_append_entries(
            &mut raft_state,
            &mut state_storage,
            &mut log_storage,
            request,
        )
        .unwrap();

        assert!(response.success);
        assert_eq!(log_storage.last_index(), 1);
    }

    #[test]
    fn test_append_entries_reject_stale_term() {
        let temp_dir = TempDir::new().unwrap();
        let mut raft_state = RaftState::new("node-1".to_string());
        raft_state.current_term = 2;

        let mut state_storage = FileStateStorage::new(temp_dir.path().join("state")).unwrap();
        let mut log_storage = FileLogStorage::new(temp_dir.path().join("logs")).unwrap();

        let request = AppendEntriesRequest {
            term: 1,
            leader_id: "node-2".to_string(),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let response = handle_append_entries(
            &mut raft_state,
            &mut state_storage,
            &mut log_storage,
            request,
        )
        .unwrap();

        assert!(!response.success);
        assert_eq!(response.term, 2);
    }
}
