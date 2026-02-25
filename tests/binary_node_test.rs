use std::collections::HashMap;

use bytes::Bytes;

use chatwarp_api::wa::binary_node::{BinaryNode, NodeContent, decode, encode};

#[test]
fn decode_message_fixture() -> anyhow::Result<()> {
    let fixture = std::fs::read("tests/fixtures/binary_node_synthetic/message_text.bin")?;
    let node = decode(&fixture)?;

    assert_eq!(node.tag, "message");
    assert_eq!(node.attrs.get("to"), Some(&"123@s.whatsapp.net".to_string()));
    assert_eq!(node.attrs.get("type"), Some(&"text".to_string()));

    match node.content {
        NodeContent::Bytes(payload) => assert_eq!(payload.as_ref(), b"hello"),
        other => anyhow::bail!("expected bytes content, got {other:?}"),
    }

    Ok(())
}

#[test]
fn decode_nested_fixture() -> anyhow::Result<()> {
    let fixture = std::fs::read("tests/fixtures/binary_node_synthetic/nested_items.bin")?;
    let node = decode(&fixture)?;

    assert_eq!(node.tag, "metadata");

    let NodeContent::Nodes(children) = node.content else {
        anyhow::bail!("expected nested children in metadata");
    };

    assert_eq!(children.len(), 2);
    assert_eq!(children[0].tag, "item");
    assert_eq!(children[0].attrs.get("type"), Some(&"alpha".to_string()));
    assert!(matches!(children[0].content, NodeContent::Empty));

    assert_eq!(children[1].tag, "item");
    assert_eq!(children[1].attrs.get("type"), Some(&"beta".to_string()));
    match &children[1].content {
        NodeContent::Bytes(payload) => assert_eq!(payload.as_ref(), b"xyz"),
        other => anyhow::bail!("expected bytes content, got {other:?}"),
    }

    Ok(())
}

#[test]
fn encode_decode_roundtrip() -> anyhow::Result<()> {
    let mut root_attrs = HashMap::new();
    root_attrs.insert("id".to_string(), "n-1".to_string());

    let mut child_attrs = HashMap::new();
    child_attrs.insert("type".to_string(), "text".to_string());

    let root = BinaryNode {
        tag: "metadata".to_string(),
        attrs: root_attrs,
        content: NodeContent::Nodes(vec![BinaryNode {
            tag: "item".to_string(),
            attrs: child_attrs,
            content: NodeContent::Bytes(Bytes::from_static(b"payload")),
        }]),
    };

    let encoded = encode(&root)?;
    let decoded = decode(&encoded)?;
    assert_eq!(decoded, root);

    Ok(())
}

#[test]
fn fixture_decode_encode_decode_is_stable() -> anyhow::Result<()> {
    let fixture = std::fs::read("tests/fixtures/binary_node_synthetic/message_text.bin")?;
    let decoded_once = decode(&fixture)?;
    let encoded = encode(&decoded_once)?;
    let decoded_twice = decode(&encoded)?;
    assert_eq!(decoded_once, decoded_twice);
    Ok(())
}
