use std::time::Duration;

use actix::prelude::*;

use crate::network::actor::Network;

pub struct Consensus {
    network_addr: Option<Addr<Network>>,
}

/// Message to set the network address
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetNetworkAddr(pub Addr<Network>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct NetworkMessage(pub String);

impl Actor for Consensus {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        //TODO: start consensus layer
        println!("Starting consensus layer");

        ctx.run_interval(Duration::from_secs(5), |actor, ctx| {
            if let Some(network_addr) = &actor.network_addr {
                network_addr.do_send(crate::network::actor::ConsensusMessage(vec![]));
            }
        });
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

impl Handler<NetworkMessage> for Consensus {
    type Result = ();

    fn handle(&mut self, msg: NetworkMessage, ctx: &mut Self::Context) -> Self::Result {
        println!("Received data from network: {}", msg.0);
    }
}
