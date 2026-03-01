use std::collections::HashMap;

use bytes::Bytes;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;

use chatwarp_api::wa::binary_node::{
    BinaryNode,
    NodeContent,
    decode,
    decode_real,
    encode,
    encode_real,
};

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

#[test]
fn real_codec_roundtrip_with_double_tokens_and_jid() -> anyhow::Result<()> {
    let mut root_attrs = HashMap::new();
    root_attrs.insert("id".to_owned(), "19".to_owned());
    root_attrs.insert("to".to_owned(), "s.whatsapp.net".to_owned());
    root_attrs.insert("participant".to_owned(), "12345:2@s.whatsapp.net".to_owned());
    root_attrs.insert("code".to_owned(), "1A2B".to_owned());
    root_attrs.insert("t".to_owned(), "123-45.6".to_owned());

    let root = BinaryNode {
        tag: "iq".to_owned(),
        attrs: root_attrs,
        content: NodeContent::Nodes(vec![BinaryNode {
            tag: "pair-device".to_owned(),
            attrs: HashMap::new(),
            content: NodeContent::Nodes(vec![BinaryNode {
                tag: "ref".to_owned(),
                attrs: HashMap::new(),
                content: NodeContent::Bytes(Bytes::from_static(b"2@test-ref")),
            }]),
        }]),
    };

    let encoded = encode_real(&root)?;
    let decoded = decode_real(&encoded)?;
    assert_eq!(decoded, root);
    Ok(())
}

#[test]
fn real_codec_supports_compressed_payload() -> anyhow::Result<()> {
    let node = BinaryNode {
        tag: "success".to_owned(),
        attrs: HashMap::new(),
        content: NodeContent::Empty,
    };
    let encoded = encode_real(&node)?;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&encoded[1..])?;
    let compressed_body = encoder.finish()?;

    let mut compressed = Vec::with_capacity(1 + compressed_body.len());
    compressed.push(0x02);
    compressed.extend_from_slice(&compressed_body);

    let decoded = decode_real(&compressed)?;
    assert_eq!(decoded, node);
    Ok(())
}
