use async_trait::async_trait;

use futures::{future::poll_fn, StreamExt};
use libp2p::{
    mdns, noise, ping,
    request_response::{self, Codec, ProtocolSupport, ResponseChannel},
    swarm::NetworkBehaviour,
    tcp, yamux, Multiaddr, PeerId, Swarm,
};
use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind};
use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::{io, select};
// Define a simple protocol
#[derive(Debug, Clone)]
struct MessageProtocol;

// Define request and response types
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageRequest(String);

#[derive(Debug, Clone, Serialize, Deserialize)]
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
                    Err(e) => return Err(e),
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
    Ok(())

    // You need to await the startup function to execute the future it returns
}
