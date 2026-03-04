use super::parse_chatstate;
use crate::types::presence::{ChatPresence, ChatPresenceMedia};
use warp_core_binary::builder::NodeBuilder;

#[test]
fn test_parse_chatstate_composing_text() {
    let node = NodeBuilder::new("chatstate")
        .children([NodeBuilder::new("composing").build()])
        .build();

    let (state, media) = parse_chatstate(&node).expect("should parse composing");
    assert_eq!(state, ChatPresence::Composing);
    assert_eq!(media, ChatPresenceMedia::Text);
}

#[test]
fn test_parse_chatstate_composing_audio() {
    let node = NodeBuilder::new("chatstate")
        .children([NodeBuilder::new("composing").attr("media", "audio").build()])
        .build();

    let (state, media) = parse_chatstate(&node).expect("should parse composing audio");
    assert_eq!(state, ChatPresence::Composing);
    assert_eq!(media, ChatPresenceMedia::Audio);
}

#[test]
fn test_parse_chatstate_paused() {
    let node = NodeBuilder::new("chatstate")
        .children([NodeBuilder::new("paused").build()])
        .build();

    let (state, media) = parse_chatstate(&node).expect("should parse paused");
    assert_eq!(state, ChatPresence::Paused);
    assert_eq!(media, ChatPresenceMedia::Text);
}

#[test]
fn test_parse_chatstate_unsupported() {
    let node = NodeBuilder::new("chatstate")
        .children([NodeBuilder::new("unknown").build()])
        .build();

    assert!(parse_chatstate(&node).is_none());
}
