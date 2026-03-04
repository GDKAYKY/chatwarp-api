use super::traits::StanzaHandler;
use crate::client::Client;
use crate::types::events::{ChatPresenceUpdate, Event};
use crate::types::presence::{ChatPresence, ChatPresenceMedia};
use async_trait::async_trait;
use log::warn;
use std::sync::Arc;
use warp_core_binary::jid::JidExt;
use warp_core_binary::node::Node;

/// Handler for `<chatstate>` stanzas.
///
/// Processes typing/recording/paused indicators and dispatches them as
/// `Event::ChatPresence`.
#[derive(Default)]
pub struct ChatstateHandler;

#[async_trait]
impl StanzaHandler for ChatstateHandler {
    fn tag(&self) -> &'static str {
        "chatstate"
    }

    async fn handle(&self, client: Arc<Client>, node: Arc<Node>, _cancelled: &mut bool) -> bool {
        let mut attrs = node.attrs();
        let from = match attrs.optional_jid("from") {
            Some(from) => from,
            None => {
                warn!(target: "Client", "Ignoring malformed <chatstate> without 'from'");
                return true;
            }
        };
        let participant = attrs.optional_jid("participant");

        let (state, media) = match parse_chatstate(&node) {
            Some(parsed) => parsed,
            None => {
                warn!(target: "Client", "Ignoring <chatstate> with unsupported payload");
                return true;
            }
        };

        let sender = if from.is_group() {
            participant.unwrap_or_else(|| from.clone())
        } else {
            from.clone()
        };

        let source = crate::types::message::MessageSource {
            chat: from.clone(),
            sender,
            is_group: from.is_group(),
            ..Default::default()
        };

        client
            .core
            .event_bus
            .dispatch(&Event::ChatPresence(ChatPresenceUpdate {
                source,
                state,
                media,
            }));

        true
    }
}

fn parse_chatstate(node: &Node) -> Option<(ChatPresence, ChatPresenceMedia)> {
    let children = node.children()?;
    let child = children.first()?;

    match child.tag.as_str() {
        "composing" => {
            let media = match child.attrs().optional_string("media") {
                Some("audio") => ChatPresenceMedia::Audio,
                _ => ChatPresenceMedia::Text,
            };
            Some((ChatPresence::Composing, media))
        }
        "paused" => Some((ChatPresence::Paused, ChatPresenceMedia::Text)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/tests/handlers/chatstate_tests.rs"
    ));
}
