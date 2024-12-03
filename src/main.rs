use futures::StreamExt;
use libp2p::{
    gossipsub, mdns, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm,
};
use std::{
    collections::hash_map::DefaultHasher,
    error::Error,
    hash::{Hash, Hasher},
};
use std::{collections::HashMap, time::Duration};

use tokio::{io, io::AsyncBufReadExt, select};

#[derive(NetworkBehaviour)]
struct Behaviour {
    mdns: mdns::tokio::Behaviour,
}

async fn try_dial_peer(swarm: &mut Swarm<Behaviour>, peer_address: Multiaddr) {
    if let Err(e) = swarm.dial(peer_address.clone()) {
        println!("Dialing failed: {e}");
    } else {
        println!("Successfully dialed peer: {peer_address}");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            let mdns =
                mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;
            Ok(Behaviour { mdns })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let mut peer_streams: HashMap<PeerId, (Multiaddr, tokio::io::DuplexStream)> = HashMap::new();

    loop {
        select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, peer_address) in list {
                        println!("mDNS discovered a new peer: {peer_id}");

                        try_dial_peer(&mut swarm, peer_address.clone()).await;

                    
                        let _ = swarm.select_next_some().await;
                        //TODO:

                    }
                },
                SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discover peer has expired: {peer_id}");
                    }
                },

                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Local node is listening on {address}");
                }
                _ => {}
            }
        }
    }
    // You need to await the startup function to execute the future it returns
}
