use super::rpc::{RequestVoteRequest, RequestVoteResponse};
use super::state::RaftState;
use super::types::{NodeId, Term};
use crate::storage::{LogStorage, StateStorage};
use crate::util::errors::Result;

/// Handle incoming RequestVote RPC
pub fn handle_request_vote<L: LogStorage + ?Sized, S: StateStorage + ?Sized>(
    raft_state: &mut RaftState,
    state_storage: &mut S,
    log_storage: &L,
    request: RequestVoteRequest,
) -> Result<RequestVoteResponse> {
    tracing::debug!(
        "Node {} received RequestVote from {} (term: {})",
        raft_state.node_id,
        request.candidate_id,
        request.term
    );

    // If request term is greater, update our term and become follower
    if request.term > raft_state.current_term {
        raft_state.update_term(request.term);
        state_storage.save_term(raft_state.current_term)?;
        state_storage.save_voted_for(None)?;
    }

    let mut vote_granted = false;

    // Grant vote if:
    // 1. Candidate's term is at least as current as ours
    // 2. We haven't voted yet in this term, or we already voted for this candidate
    // 3. Candidate's log is at least as up-to-date as ours
    if request.term >= raft_state.current_term {
        let can_vote = raft_state.voted_for.is_none()
            || raft_state.voted_for.as_ref() == Some(&request.candidate_id);

        if can_vote {
            // Check if candidate's log is at least as up-to-date as ours
            let last_log_term = log_storage.last_term();
            let last_log_index = log_storage.last_index();

            let log_is_up_to_date = request.last_log_term > last_log_term
                || (request.last_log_term == last_log_term
                    && request.last_log_index >= last_log_index);

            if log_is_up_to_date {
                vote_granted = true;
                raft_state.voted_for = Some(request.candidate_id.clone());
                state_storage.save_voted_for(raft_state.voted_for.clone())?;

                tracing::info!(
                    "Node {} granted vote to {} in term {}",
                    raft_state.node_id,
                    request.candidate_id,
                    request.term
                );
            } else {
                tracing::debug!(
                    "Node {} denied vote to {} - log not up-to-date",
                    raft_state.node_id,
                    request.candidate_id
                );
            }
        } else {
            tracing::debug!(
                "Node {} denied vote to {} - already voted for {:?}",
                raft_state.node_id,
                request.candidate_id,
                raft_state.voted_for
            );
        }
    } else {
        tracing::debug!(
            "Node {} denied vote to {} - request term {} < current term {}",
            raft_state.node_id,
            request.candidate_id,
            request.term,
            raft_state.current_term
        );
    }

    Ok(RequestVoteResponse {
        term: raft_state.current_term,
        vote_granted,
    })
}

/// Handle incoming RequestVote response
pub fn handle_request_vote_response(
    raft_state: &mut RaftState,
    from: NodeId,
    response: RequestVoteResponse,
    total_nodes: usize,
) -> Result<bool> {
    // If response term is greater, update our term and become follower
    if response.term > raft_state.current_term {
        raft_state.update_term(response.term);
        return Ok(false);
    }

    // Ignore if we're not a candidate anymore
    if !raft_state.is_candidate() {
        return Ok(false);
    }

    // Ignore stale responses
    if response.term < raft_state.current_term {
        return Ok(false);
    }

    // Record the vote if granted
    if response.vote_granted {
        raft_state.add_vote(from.clone());

        tracing::debug!(
            "Node {} received vote from {} ({}/{} votes)",
            raft_state.node_id,
            from,
            raft_state.votes_received.len(),
            total_nodes
        );

        // Check if we won the election
        if raft_state.has_majority(total_nodes) {
            tracing::info!(
                "Node {} won election in term {} with {} votes",
                raft_state.node_id,
                raft_state.current_term,
                raft_state.votes_received.len()
            );
            return Ok(true); // We became leader
        }
    }

    Ok(false)
}

/// Create a RequestVote request for this node
pub fn create_request_vote<L: LogStorage + ?Sized>(
    raft_state: &RaftState,
    log_storage: &L,
) -> RequestVoteRequest {
    RequestVoteRequest {
        term: raft_state.current_term,
        candidate_id: raft_state.node_id.clone(),
        last_log_index: log_storage.last_index(),
        last_log_term: log_storage.last_term(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raft::types::LogEntry;
    use crate::storage::{FileLogStorage, FileStateStorage};
    use tempfile::TempDir;

    #[test]
    fn test_grant_vote_to_candidate() {
        let temp_dir = TempDir::new().unwrap();
        let mut raft_state = RaftState::new("node-1".to_string());
        let mut state_storage = FileStateStorage::new(temp_dir.path().join("node-1")).unwrap();
        let log_storage = FileLogStorage::new(temp_dir.path().join("logs")).unwrap();

        let request = RequestVoteRequest {
            term: 1,
            candidate_id: "node-2".to_string(),
            last_log_index: 0,
            last_log_term: 0,
        };

        let response =
            handle_request_vote(&mut raft_state, &mut state_storage, &log_storage, request)
                .unwrap();

        assert!(response.vote_granted);
        assert_eq!(raft_state.voted_for, Some("node-2".to_string()));
    }

    #[test]
    fn test_deny_vote_if_already_voted() {
        let temp_dir = TempDir::new().unwrap();
        let mut raft_state = RaftState::new("node-1".to_string());
        raft_state.current_term = 1;
        raft_state.voted_for = Some("node-2".to_string());

        let mut state_storage = FileStateStorage::new(temp_dir.path().join("node-1")).unwrap();
        let log_storage = FileLogStorage::new(temp_dir.path().join("logs")).unwrap();

        let request = RequestVoteRequest {
            term: 1,
            candidate_id: "node-3".to_string(),
            last_log_index: 0,
            last_log_term: 0,
        };

        let response =
            handle_request_vote(&mut raft_state, &mut state_storage, &log_storage, request)
                .unwrap();

        assert!(!response.vote_granted);
    }
}
