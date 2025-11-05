use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::request_response::Codec;
use serde::{Deserialize, Serialize};
use std::{
    io::{Error, ErrorKind},
    pin::Pin,
};

use crate::raft::rpc::RaftMessage;

// Constants for message size management
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB - prevent DoS attacks

// Network message envelope that can carry different types of messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    Raft(RaftMessage),
    Heartbeat,
    Test(i64), // chat_id for Telegram bot test command
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRequest(pub NetworkMessage);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse(pub NetworkMessage);

#[derive(Clone, Default)]
pub struct MessageCodec;

/// Get bincode configuration for consistent encoding/decoding
fn bincode_config() -> bincode::config::Configuration {
    bincode::config::standard().with_variable_int_encoding()
}

/// Serialize with size limit to prevent attacks using serde compatibility
fn serialize_with_limit<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let config = bincode_config();

    // Use bincode's serde compatibility layer
    let encoded = bincode::serde::encode_to_vec(value, config)
        .map_err(|e| format!("Serialization error: {}", e))?;

    // Validate size after encoding
    if encoded.len() > MAX_MESSAGE_SIZE {
        return Err("Encoded message exceeds size limit".to_string());
    }

    Ok(encoded)
}

/// Deserialize with validation using serde compatibility
fn deserialize_with_limit<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    if bytes.len() > MAX_MESSAGE_SIZE {
        return Err("Message size exceeds limit".to_string());
    }

    let config = bincode_config();

    // Use bincode's serde compatibility layer
    let (value, _): (T, usize) = bincode::serde::decode_from_slice(bytes, config)
        .map_err(|e| format!("Deserialization error: {}", e))?;

    Ok(value)
}

impl Codec for MessageCodec {
    type Protocol = &'static str;
    type Request = MessageRequest;
    type Response = MessageResponse;

    fn read_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
    ) -> Pin<
        Box<
            dyn core::future::Future<Output = std::io::Result<Self::Request>> + Send + 'async_trait,
        >,
    >
    where
        T: AsyncRead + Unpin + Send + 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async { Err(Error::new(ErrorKind::InvalidData, "Invalid protocol")) });
        }

        Box::pin(async move {
            // Read 4-byte length prefix to know exact message size
            let mut len_bytes = [0u8; 4];
            io.read_exact(&mut len_bytes).await?;
            let len = u32::from_be_bytes(len_bytes) as usize;

            // Validate message size to prevent DoS attacks
            if len > MAX_MESSAGE_SIZE {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("Message size {} exceeds maximum {}", len, MAX_MESSAGE_SIZE),
                ));
            }

            if len == 0 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Message size cannot be zero",
                ));
            }

            // Read exact payload size
            let mut buffer = vec![0u8; len];
            io.read_exact(&mut buffer).await?;

            // Deserialize with validation
            deserialize_with_limit::<MessageRequest>(&buffer).map_err(|e| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("Deserialization error: {}", e),
                )
            })
        })
    }

    fn write_request<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
        req: Self::Request,
    ) -> Pin<Box<dyn core::future::Future<Output = std::io::Result<()>> + Send + 'async_trait>>
    where
        T: AsyncWrite + Unpin + Send + 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async { Err(Error::new(ErrorKind::InvalidData, "Invalid protocol")) });
        }

        Box::pin(async move {
            // Serialize with validation
            let bytes = serialize_with_limit(&req).map_err(|e| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("Serialization error: {}", e),
                )
            })?;

            // Validate serialized size
            if bytes.len() > MAX_MESSAGE_SIZE {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("Serialized message size {} exceeds maximum", bytes.len()),
                ));
            }

            // Write 4-byte length prefix + payload
            let len = bytes.len() as u32;
            io.write_all(&len.to_be_bytes()).await?;
            io.write_all(&bytes).await?;
            io.flush().await?;
            Ok(())
        })
    }

    fn read_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
    ) -> Pin<
        Box<
            dyn core::future::Future<Output = std::io::Result<Self::Response>>
                + Send
                + 'async_trait,
        >,
    >
    where
        T: AsyncRead + Unpin + Send + 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async { Err(Error::new(ErrorKind::InvalidData, "Invalid protocol")) });
        }

        Box::pin(async move {
            // Read 4-byte length prefix to know exact message size
            let mut len_bytes = [0u8; 4];
            io.read_exact(&mut len_bytes).await?;
            let len = u32::from_be_bytes(len_bytes) as usize;

            // Validate message size to prevent DoS attacks
            if len > MAX_MESSAGE_SIZE {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("Response size {} exceeds maximum {}", len, MAX_MESSAGE_SIZE),
                ));
            }

            if len == 0 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Response size cannot be zero",
                ));
            }

            // Read exact payload size
            let mut buffer = vec![0u8; len];
            io.read_exact(&mut buffer).await?;

            // Deserialize with validation
            deserialize_with_limit::<MessageResponse>(&buffer).map_err(|e| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("Deserialization error: {}", e),
                )
            })
        })
    }

    fn write_response<'life0, 'life1, 'life2, 'async_trait, T>(
        &'life0 mut self,
        protocol: &'life1 Self::Protocol,
        io: &'life2 mut T,
        res: Self::Response,
    ) -> Pin<Box<dyn core::future::Future<Output = std::io::Result<()>> + Send + 'async_trait>>
    where
        T: AsyncWrite + Unpin + Send + 'async_trait,
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        if *protocol != "/message_protocol/1" {
            return Box::pin(async { Err(Error::new(ErrorKind::InvalidData, "Invalid protocol")) });
        }

        Box::pin(async move {
            // Serialize with validation
            let bytes = serialize_with_limit(&res).map_err(|e| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("Serialization error: {}", e),
                )
            })?;

            // Validate serialized size
            if bytes.len() > MAX_MESSAGE_SIZE {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("Serialized response size {} exceeds maximum", bytes.len()),
                ));
            }

            // Write 4-byte length prefix + payload
            let len = bytes.len() as u32;
            io.write_all(&len.to_be_bytes()).await?;
            io.write_all(&bytes).await?;
            io.flush().await?;
            Ok(())
        })
    }
}
