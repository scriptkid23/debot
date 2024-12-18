use actix::prelude::*;

pub struct Consensus {
}

impl Actor for Consensus {
    type Context = Context<Self>;
}
