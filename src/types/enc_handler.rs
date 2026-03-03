use crate::client::Client;
use crate::types::message::MessageInfo;
use anyhow::Result;
use std::sync::Arc;
use warp_core_binary::node::Node;

/// Trait for handling custom encrypted message types
#[async_trait::async_trait]
pub trait EncHandler: Send + Sync {
    /// Handle an encrypted node of a specific type
    ///
    /// # Arguments
    /// * `client` - The client instance
    /// * `enc_node` - The encrypted node to handle
    /// * `info` - The message info context
    ///
    /// # Returns
    /// * `Ok(())` if the message was handled successfully
    /// * `Err(anyhow::Error)` if handling failed
    async fn handle(&self, client: Arc<Client>, enc_node: &Node, info: &MessageInfo) -> Result<()>;
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tests/types/enc_handler_tests.rs"));
}
