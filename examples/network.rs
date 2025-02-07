use std::collections::HashSet;

use futures::prelude::*;
use futures::{future::poll_fn, StreamExt};
use libp2p::request_response::Config;
use libp2p::swarm::SwarmEvent;
use libp2p::{
    mdns, noise,
    request_response::{self, Codec, ProtocolSupport},
    swarm::NetworkBehaviour,
    tcp, yamux, Multiaddr, PeerId, Swarm,
};
use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind};
use std::{pin::Pin, time::Duration};

use tokio::{io, select};
use tokio_util::codec::{FramedRead, LinesCodec};

// Define request and response types
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageRequest(String);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageResponse(String);

// Implement the codec for our protocol
#[derive(Clone, Default)]
struct MessageCodec;

impl Codec for MessageCodec {
    type Protocol = &'static str;
    type Request = MessageRequest;
    type Response = MessageResponse;

    fn read_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = std::io::Result<Self::Response>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        T: futures::AsyncRead + Unpin + Send,
        T: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid protocol",
                ))
            });
        }

        Box::pin(async move {
            let mut buffer = vec![0u8; 1024];
            let mut total_size = 0;

            loop {
                let poll_result =
                    poll_fn(|cx| Pin::new(&mut *io).poll_read(cx, &mut buffer[total_size..])).await;

                match poll_result {
                    Ok(size) => {
                        if size == 0 {
                            return Err(Error::new(
                                ErrorKind::UnexpectedEof,
                                "Connection closed while reading response",
                            ));
                        }
                        total_size += size;

                        if let Ok(response) =
                            bincode::deserialize::<MessageResponse>(&buffer[..total_size])
                        {
                            return Ok(response);
                        }
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        })
    }

    fn write_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
        req: Self::Request,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = std::io::Result<()>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        T: futures::AsyncWrite + Unpin + Send,
        T: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid protocol",
                ))
            });
        }

        Box::pin(async move {
            let bytes = bincode::serialize(&req)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

            io.write_all(&bytes).await?;
            io.flush().await?;
            Ok(())
        })
    }
    fn write_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
        res: Self::Response,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = std::io::Result<()>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        T: futures::AsyncWrite + Unpin + Send,
        T: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid protocol",
                ))
            });
        }
        Box::pin(async move {
            let bytes = bincode::serialize(&res)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;
            // Write the serialized response
            io.write_all(&bytes).await?;

            // Flush the writer
            io.flush().await?;

            Ok(())
        })
    }
    fn read_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = std::io::Result<Self::Request>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        T: futures::AsyncRead + Unpin + Send,
        T: 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid protocol",
                ))
            });
        }
        Box::pin(async move {
            let mut buffer = vec![0u8; 1024];
            let mut total_size = 0;

            loop {
                let poll_result = poll_fn(|cx| {
                    // Borrow `io` explicitly and ensure it is pinned
                    Pin::new(&mut *io).poll_read(cx, &mut buffer[total_size..])
                })
                .await;

                match poll_result {
                    Ok(size) => {
                        if size == 0 {
                            return Err(Error::new(ErrorKind::UnexpectedEof, "Connection closed"));
                        }
                        total_size += size;

                        // Attempt to deserialize
                        if let Ok(request) =
                            bincode::deserialize::<MessageRequest>(&buffer[..total_size])
                        {
                            return Ok(request);
                        }
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        })
    }
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    mdns: mdns::tokio::Behaviour,
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
    // Generate a random keypair for this peer.
    let id_keys = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(id_keys.public());
    println!("Local peer id: {peer_id}");

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

            let protocols = vec![("/message_protocol/1", ProtocolSupport::Full)];

            let config = Config::default();
            let request_response = request_response::Behaviour::new(protocols, config);

            Ok(Behaviour {
                mdns,
                request_response,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Đọc stdin
    let stdin = tokio::io::stdin();
    let mut stdin = FramedRead::new(stdin, LinesCodec::new());

    println!("Type 'send <message>' to send a message to peers:");

    loop {
        select! {
            line = stdin.next() => match line {
                Some(Ok(line)) => {
                    if let Some(message) = line.strip_prefix("send ") {
                        println!("Sending...: {}", message);
                        let peers: HashSet<PeerId> = swarm.behaviour().mdns.discovered_nodes().cloned().collect();

                        println!("{:?}", peers);


                        if peers.is_empty() {
                                print!("Nothing!")
                        }else {
                            for peer in peers {
                                println!("{:?}", swarm.is_connected(&peer));
                                let request = MessageRequest(message.to_string());
                                swarm.behaviour_mut().request_response.send_request(&peer, request);
                            }
                        }
                    }
                }
                Some(Err(e)) => eprintln!("Error reading from stdin: {:?}", e),
                None => break,
            },

            //TODO: send a message and another peer will receive and show the message
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, multiaddr) in list {
                        println!("mDNS discovered a new peer: {peer_id}");
                        try_dial_peer(&mut swarm, multiaddr).await;
                    }
                },

                SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discover peer has expired: {peer_id}");
                    }
                },

                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    println!("Connection established with {peer_id}");
                }

                SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(event)) => {
                    match event {
                        request_response::Event::Message { peer, message } => {
                            match message {
                                request_response::Message::Request { request, channel, .. } => {
                                    println!("Received request from {}: {:?}", peer, request);

                                    let response = MessageResponse("Response to your request".to_string());
                                    swarm.behaviour_mut().request_response.send_response(channel, response).unwrap();
                                }
                                request_response::Message::Response { request_id, response } => {
                                    println!("Received response for request {}: {:?}", request_id, response);

                                }
                            }
                        }
                        request_response::Event::OutboundFailure { peer, request_id, error } => {
                            println!("Outbound failure for request {} to {}: {:?}", request_id, peer, error);
                        }
                        request_response::Event::InboundFailure { peer, request_id, error } => {
                            println!("Inbound failure from {} for request {}: {:?}", peer, request_id, error);
                        }
                        request_response::Event::ResponseSent { peer, request_id } => {
                            println!("Response sent to {} for request {}", peer, request_id);
                        }
                    }
                },

                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Local node is listening on {address}");
                }
                _ => {}
            }
        }
    }
    Ok(())
    // You need to await the startup function to execute the future it returns
}
