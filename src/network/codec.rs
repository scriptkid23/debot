use futures::prelude::*;
use futures::{ future::poll_fn, AsyncRead, AsyncWrite };
use libp2p::request_response::Codec;
use serde::{ Deserialize, Serialize };
use std::{ io::{ Error, ErrorKind }, pin::Pin };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRequest(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse(pub String);

#[derive(Clone, Default)]
pub struct MessageCodec;

impl Codec for MessageCodec {
    type Protocol = &'static str;
    type Request = MessageRequest;
    type Response = MessageResponse;

    fn read_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T
    )
        -> Pin<
            Box<
                dyn core::future::Future<Output = std::io::Result<Self::Request>> +
                    Send +
                    'async_trait
            >
        >
        where
            T: AsyncRead + Unpin + Send + 'async_trait,
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
            Self: 'async_trait
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async {
                Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid protocol"))
            });
        }

        Box::pin(async move {
            let mut buffer = vec![0u8; 1024];
            let mut total_size = 0;

            loop {
                let poll_result = poll_fn(|cx|
                    Pin::new(&mut *io).poll_read(cx, &mut buffer[total_size..])
                ).await;

                match poll_result {
                    Ok(size) => {
                        if size == 0 {
                            return Err(Error::new(ErrorKind::UnexpectedEof, "Connection closed"));
                        }
                        total_size += size;

                        if
                            let Ok(request) = bincode::deserialize::<MessageRequest>(
                                &buffer[..total_size]
                            )
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

    fn write_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
        req: Self::Request
    )
        -> Pin<Box<dyn core::future::Future<Output = std::io::Result<()>> + Send + 'async_trait>>
        where
            T: AsyncWrite + Unpin + Send + 'async_trait,
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
            Self: 'async_trait
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async {
                Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid protocol"))
            });
        }

        Box::pin(async move {
            let bytes = bincode
                ::serialize(&req)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

            io.write_all(&bytes).await?;
            io.flush().await?;
            Ok(())
        })
    }

    fn read_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T
    )
        -> Pin<
            Box<
                dyn core::future::Future<Output = std::io::Result<Self::Response>> +
                    Send +
                    'async_trait
            >
        >
        where
            T: AsyncRead + Unpin + Send + 'async_trait,
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
            Self: 'async_trait
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async {
                Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid protocol"))
            });
        }

        Box::pin(async move {
            let mut buffer = vec![0u8; 1024];
            let mut total_size = 0;

            loop {
                let poll_result = poll_fn(|cx|
                    Pin::new(&mut *io).poll_read(cx, &mut buffer[total_size..])
                ).await;

                match poll_result {
                    Ok(size) => {
                        if size == 0 {
                            return Err(
                                Error::new(
                                    ErrorKind::UnexpectedEof,
                                    "Connection closed while reading response"
                                )
                            );
                        }
                        total_size += size;

                        if
                            let Ok(response) = bincode::deserialize::<MessageResponse>(
                                &buffer[..total_size]
                            )
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

    fn write_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
        res: Self::Response
    )
        -> Pin<Box<dyn core::future::Future<Output = std::io::Result<()>> + Send + 'async_trait>>
        where
            T: AsyncWrite + Unpin + Send + 'async_trait,
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
            Self: 'async_trait
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async {
                Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid protocol"))
            });
        }
        Box::pin(async move {
            let bytes = bincode
                ::serialize(&res)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;
            io.write_all(&bytes).await?;
            io.flush().await?;
            Ok(())
        })
    }
}
