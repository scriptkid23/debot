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

#[derive(Debug)]
pub enum Network2ConsensusRequestError {
    InvalidData,
    // Add other error variants as needed
}

#[derive(Message)]
#[rtype(result = "Result<String, Network2ConsensusRequestError>")]
pub struct Network2ConsensusRequest(pub String);

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

impl Handler<Network2ConsensusRequest> for Consensus {
    type Result = Result<String, Network2ConsensusRequestError>;

    fn handle(&mut self, msg: Network2ConsensusRequest, ctx: &mut Self::Context) -> Self::Result {
        println!("Received data from network: {}", msg.0);
        Ok("Processed network event".to_string())
    }
}
