use actix::prelude::*;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::client::commands::CommandRegistry;
use crate::client::messages::{
    BroadcastCommand, GetNodeStatus, NodeStatus, SetConsensusAddr, SubmitTransaction,
    TransactionResult, TransactionStatus,
};
use crate::consensus::actor::{Consensus, ProposeCommand};

const BATCH_WINDOW_MS: u64 = 10; // 10ms batching window for high throughput
const MAX_BATCH_SIZE: usize = 100; // Maximum transactions per batch

pub struct Client {
    bot_id: String,
    consensus_addr: Option<Addr<Consensus>>,
    pending_transactions: VecDeque<PendingTransaction>,
    total_transactions: u64,
    batch_timer: Option<SpawnHandle>,
    last_batch_time: Instant,
    command_registry: CommandRegistry,
}

#[allow(dead_code)]
struct PendingTransaction {
    tx_id: u64,
    data: Vec<u8>,
    submitted_at: Instant,
}

impl Client {
    pub fn new(bot_id: String) -> Self {
        Client {
            bot_id,
            consensus_addr: None,
            pending_transactions: VecDeque::new(),
            total_transactions: 0,
            batch_timer: None,
            last_batch_time: Instant::now(),
            command_registry: CommandRegistry::with_defaults(),
        }
    }

    pub fn with_command_registry(bot_id: String, registry: CommandRegistry) -> Self {
        Client {
            bot_id,
            consensus_addr: None,
            pending_transactions: VecDeque::new(),
            total_transactions: 0,
            batch_timer: None,
            last_batch_time: Instant::now(),
            command_registry: registry,
        }
    }

    fn flush_batch(&mut self, ctx: &mut Context<Self>) {
        if self.pending_transactions.is_empty() {
            return;
        }

        let consensus_addr = match &self.consensus_addr {
            Some(addr) => addr.clone(),
            None => {
                tracing::warn!("Cannot flush batch: consensus address not set");
                return;
            }
        };

        let batch_size = std::cmp::min(self.pending_transactions.len(), MAX_BATCH_SIZE);
        let mut batch_data = Vec::new();

        // Collect transactions for batch
        for _ in 0..batch_size {
            if let Some(tx) = self.pending_transactions.pop_front() {
                batch_data.extend_from_slice(&tx.data);
            }
        }

        if !batch_data.is_empty() {
            tracing::debug!(
                "Flushing batch of {} transactions ({} bytes)",
                batch_size,
                batch_data.len()
            );

            // Send batch to consensus
            let fut = async move {
                match consensus_addr
                    .send(ProposeCommand { data: batch_data })
                    .await
                {
                    Ok(Ok(index)) => {
                        tracing::debug!("Batch committed at index {}", index);
                    }
                    Ok(Err(e)) => {
                        tracing::error!("Batch failed: {}", e);
                    }
                    Err(e) => {
                        tracing::error!("Failed to send batch: {}", e);
                    }
                }
            };

            ctx.spawn(fut.into_actor(self));
        }

        self.last_batch_time = Instant::now();
    }

    fn schedule_batch_flush(&mut self, ctx: &mut Context<Self>) {
        // Cancel existing timer if any
        if let Some(handle) = self.batch_timer.take() {
            ctx.cancel_future(handle);
        }

        // Schedule new batch flush
        let handle = ctx.run_later(Duration::from_millis(BATCH_WINDOW_MS), |act, ctx| {
            act.flush_batch(ctx);
            act.batch_timer = None;
        });

        self.batch_timer = Some(handle);
    }
}

impl Actor for Client {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        tracing::info!("Client actor started for bot: {}", self.bot_id);
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        tracing::info!("Client actor stopped for bot: {}", self.bot_id);
        // Flush any pending transactions before stopping
        self.flush_batch(ctx);
    }
}

impl Handler<SetConsensusAddr> for Client {
    type Result = ();

    fn handle(&mut self, msg: SetConsensusAddr, _ctx: &mut Self::Context) -> Self::Result {
        self.consensus_addr = Some(msg.0);
        tracing::info!("Consensus address set for bot: {}", self.bot_id);
    }
}

impl Handler<SubmitTransaction> for Client {
    type Result = ResponseFuture<Result<u64, String>>;

    fn handle(&mut self, msg: SubmitTransaction, ctx: &mut Self::Context) -> Self::Result {
        if self.consensus_addr.is_none() {
            return Box::pin(async { Err("Consensus not connected".to_string()) });
        }

        // Assign transaction ID
        self.total_transactions += 1;
        let tx_id = self.total_transactions;

        tracing::debug!(
            "Client {} submitting transaction {} ({} bytes)",
            msg.bot_id,
            tx_id,
            msg.data.len()
        );

        // HIGH THROUGHPUT MODE: Use batching
        // Queue the transaction for batching
        self.pending_transactions.push_back(PendingTransaction {
            tx_id,
            data: msg.data,
            submitted_at: std::time::Instant::now(),
        });

        // If batch is full, flush immediately for better throughput
        if self.pending_transactions.len() >= MAX_BATCH_SIZE {
            tracing::debug!("Batch size reached, flushing immediately");
            self.flush_batch(ctx);
            return Box::pin(async move { Ok(tx_id) });
        }

        // Schedule batch flush if not already scheduled
        if self.batch_timer.is_none() {
            self.schedule_batch_flush(ctx);
        }

        // Return immediately with tx_id (async processing)
        Box::pin(async move { Ok(tx_id) })
    }
}

impl Handler<GetNodeStatus> for Client {
    type Result = ResponseFuture<NodeStatus>;

    fn handle(&mut self, _msg: GetNodeStatus, _ctx: &mut Self::Context) -> Self::Result {
        let status = NodeStatus {
            bot_id: self.bot_id.clone(),
            is_connected: self.consensus_addr.is_some(),
            pending_transactions: self.pending_transactions.len(),
            total_transactions: self.total_transactions,
        };

        Box::pin(async move { status })
    }
}

impl Handler<TransactionResult> for Client {
    type Result = ();

    fn handle(&mut self, msg: TransactionResult, _ctx: &mut Self::Context) -> Self::Result {
        match msg.status {
            TransactionStatus::Committed => {
                tracing::info!("Transaction {} committed successfully", msg.tx_id);
            }
            TransactionStatus::Failed(ref reason) => {
                tracing::error!("Transaction {} failed: {}", msg.tx_id, reason);
            }
            TransactionStatus::Pending => {
                tracing::debug!("Transaction {} is pending", msg.tx_id);
            }
        }
    }
}

impl Handler<BroadcastCommand> for Client {
    type Result = ();

    fn handle(&mut self, msg: BroadcastCommand, _ctx: &mut Self::Context) -> Self::Result {
        tracing::info!(
            "Executing broadcast command '{}' from {} with args: {:?}",
            msg.command,
            msg.from_bot,
            msg.args
        );

        // Execute the command using the registry
        match self
            .command_registry
            .execute(&msg.command, msg.args.clone())
        {
            Some(result) => match result {
                crate::client::commands::CommandResult::Success(response) => {
                    tracing::info!(
                        "Command '{}' executed successfully: {}",
                        msg.command,
                        response
                    );
                    // The response will be sent back via Telegram by the bot manager
                }
                crate::client::commands::CommandResult::Error(err) => {
                    tracing::error!("Command '{}' failed: {}", msg.command, err);
                }
            },
            None => {
                tracing::warn!("Unknown command: {}", msg.command);
            }
        }
    }
}
