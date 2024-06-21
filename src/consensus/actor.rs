use actix::prelude::*;

pub struct ConsensusActor {}

impl Actor for ConsensusActor {
    type Context = Context<Self>;
}
