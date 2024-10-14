// src/network/mod.rs
pub mod libp2p;
pub mod peer_manager;
pub mod transport;
pub mod messages;


// Re-export NetworkMessage for easy access from outside the network module
pub use messages::NetworkMessage;