use async_trait::async_trait;

use futures::{AsyncRead, StreamExt};
use libp2p::{
    mdns, noise, ping,
    request_response::{self, Codec, ProtocolSupport, ResponseChannel},
    swarm::NetworkBehaviour,
    tcp, yamux, Multiaddr, PeerId, Swarm,
};
use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::{io, select};

// Define a simple protocol
#[derive(Debug, Clone)]
struct MessageProtocol;

// Define request and response types
#[derive(Debug, Clone)]
struct MessageRequest(String);

#[derive(Debug, Clone)]
struct MessageResponse(String);

// Implement the codec for our protocol
#[derive(Clone)]
struct MessageCodec;

impl Codec for MessageCodec {
    type Protocol = &'static str;
    type Request = MessageRequest;
    type Response = MessageResponse;

    fn read_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        _protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Request, std::io::Error>> + Send + 'async_trait>>
    where
        T: tokio::io::AsyncRead + Unpin + Send + 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let mut buf = Vec::new();

            io.read_to_end(&mut buf).await?;
            Ok(MessageRequest(String::from_utf8_lossy(&buf).into_owned()))
        })
    }
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    mdns: mdns::tokio::Behaviour,
    ping: ping::Behaviour,
    request_response: request_response::Behaviour<MessageCodec>,
}

async fn try_dial_peer(swarm: &mut Swarm<Behaviour>, peer_address: Multiaddr) {
    if let Err(e) = swarm.dial(peer_address.clone()) {
        println!("Dialing failed: {e}");
    } else {
        println!("Successfully dialed peer: {peer_address}");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

            let ping = ping::Behaviour::default();

            let request_response = request_response::Behaviour::new(protocols, cfg);
            Ok(Behaviour {
                mdns,
                ping,
                request_response,
            })
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
