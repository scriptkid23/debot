use actix::prelude::*;

use crate::network::actor::Network;

pub struct Consensus {
    network_addr: Option<Addr<Network>>,
}

/// Message to set the network address
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetNetworkAddr(pub Addr<Network>);

/// Messages to send to the network actor from the consensus layer
#[derive(Message)]
#[rtype(result = "()")]
pub struct ConsensusMessage(pub String);

impl Actor for Consensus {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("Consensus actor started");
    }
}
impl Default for Consensus {
    fn default() -> Self {
        Consensus { network_addr: None }
    }
}

impl Handler<SetNetworkAddr> for Consensus {
    type Result = ();

    fn handle(&mut self, msg: SetNetworkAddr, _ctx: &mut Self::Context) -> Self::Result {
        self.network_addr = Some(msg.0);
    }
}
