use crate::consensus::ConsensusActor;

use super::swarm::{self, DebotSwarm};
use actix::{Actor, Addr, AsyncContext, Context};
use futures::select;

pub struct NetworkActor {
    swarm: DebotSwarm,
    consensus: Addr<ConsensusActor>,
}

impl NetworkActor {
    pub fn new(swarm: DebotSwarm, consensus: Addr<ConsensusActor>) -> Self {
        NetworkActor { swarm, consensus }
    }
}

impl Actor for NetworkActor {
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        println!("NetworkActor started");
    }
}

impl NetworkActor {
    fn process_swarm_events(&mut self, ctx: &mut Context<Self>) {
        let addr = ctx.address();
        let consensus = self.consensus.clone();

        ctx.spawn(async move {
            loop {
                select! {
                    
                }
            }
        })
    }
}
