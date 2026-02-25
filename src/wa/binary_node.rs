use std::collections::HashMap;

use bytes::Bytes;

use crate::wa::error::BinaryNodeError;

/// Proprietary WA binary node structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryNode {
    /// Node tag.
    pub tag: String,
    /// Node attributes.
    pub attrs: HashMap<String, String>,
    /// Node body.
    pub content: NodeContent,
}

/// Body content carried by a [`BinaryNode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeContent {
    /// Nested child nodes.
    Nodes(Vec<BinaryNode>),
    /// Raw payload bytes.
    Bytes(Bytes),
    /// No body.
    Empty,
}

/// Decodes a binary payload into a [`BinaryNode`].
pub fn decode(input: &[u8]) -> Result<BinaryNode, BinaryNodeError> {
    let mut decoder = Decoder::new(input);
    let node = decoder.decode_node()?;

    if !decoder.is_eof() {
        return Err(BinaryNodeError::TrailingBytes);
    }

    Ok(node)
}

/// Encodes a [`BinaryNode`] into binary format.
pub fn encode(node: &BinaryNode) -> Result<Vec<u8>, BinaryNodeError> {
    let mut output = Vec::new();
    encode_node(node, &mut output)?;
    Ok(output)
}

const fn build_single_byte_tokens() -> [&'static str; 256] {
    let mut tokens = [""; 256];
    tokens[1] = "message";
    tokens[2] = "body";
    tokens[3] = "to";
    tokens[4] = "type";
    tokens[5] = "text";
    tokens[6] = "chat";
    tokens[7] = "participant";
    tokens[8] = "conversation";
    tokens[9] = "metadata";
    tokens[10] = "item";
    tokens[11] = "id";
    tokens
}

/// Token dictionary used for compact single-byte symbol encoding.
pub const SINGLE_BYTE_TOKENS: [&str; 256] = build_single_byte_tokens();

struct Decoder<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> Decoder<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }

    fn is_eof(&self) -> bool {
        self.position == self.input.len()
    }

    fn decode_node(&mut self) -> Result<BinaryNode, BinaryNodeError> {
        let tag = self.decode_symbol()?;

        let attrs_count = self.read_u16()? as usize;
        let mut attrs = HashMap::with_capacity(attrs_count);
        for _ in 0..attrs_count {
            let key = self.decode_symbol()?;
            let value = self.decode_symbol()?;
            attrs.insert(key, value);
        }

        let content = self.decode_content()?;

        Ok(BinaryNode {
            tag,
            attrs,
            content,
        })
    }

    fn decode_content(&mut self) -> Result<NodeContent, BinaryNodeError> {
        let content_type = self.read_u8()?;
        match content_type {
            0 => Ok(NodeContent::Empty),
            1 => {
                let len = self.read_u32()? as usize;
                let payload = self.read_bytes(len)?;
                Ok(NodeContent::Bytes(Bytes::copy_from_slice(payload)))
            }
            2 => {
                let count = self.read_u16()? as usize;
                let mut nodes = Vec::with_capacity(count);
                for _ in 0..count {
                    nodes.push(self.decode_node()?);
                }
                Ok(NodeContent::Nodes(nodes))
            }
            value => Err(BinaryNodeError::InvalidContentType(value)),
        }
    }

    fn decode_symbol(&mut self) -> Result<String, BinaryNodeError> {
        let symbol_type = self.read_u8()?;
        match symbol_type {
            1 => {
                let token = self.read_u8()? as usize;
                let value = SINGLE_BYTE_TOKENS[token];
                if value.is_empty() {
                    return Err(BinaryNodeError::UnknownToken(token as u8));
                }
                Ok(value.to_owned())
            }
            2 => {
                let len = self.read_u16()? as usize;
                let bytes = self.read_bytes(len)?;
                String::from_utf8(bytes.to_vec()).map_err(|_| BinaryNodeError::InvalidUtf8)
            }
            value => Err(BinaryNodeError::InvalidSymbolType(value)),
        }
    }

    fn read_u8(&mut self) -> Result<u8, BinaryNodeError> {
        let byte = self
            .input
            .get(self.position)
            .copied()
            .ok_or(BinaryNodeError::UnexpectedEof)?;
        self.position += 1;
        Ok(byte)
    }

    fn read_u16(&mut self) -> Result<u16, BinaryNodeError> {
        let raw = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([raw[0], raw[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, BinaryNodeError> {
        let raw = self.read_bytes(4)?;
        Ok(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], BinaryNodeError> {
        let end = self
            .position
            .checked_add(len)
            .ok_or(BinaryNodeError::UnexpectedEof)?;

        if end > self.input.len() {
            return Err(BinaryNodeError::UnexpectedEof);
        }

        let bytes = &self.input[self.position..end];
        self.position = end;
        Ok(bytes)
    }
}

fn encode_node(node: &BinaryNode, output: &mut Vec<u8>) -> Result<(), BinaryNodeError> {
    encode_symbol(&node.tag, output)?;

    let attrs_len = u16::try_from(node.attrs.len()).map_err(|_| BinaryNodeError::TooManyAttributes)?;
    output.extend_from_slice(&attrs_len.to_be_bytes());

    let mut keys: Vec<&String> = node.attrs.keys().collect();
    keys.sort_unstable();

    for key in keys {
        let value = node
            .attrs
            .get(key)
            .ok_or(BinaryNodeError::AttributeLookupFailed)?;
        encode_symbol(key, output)?;
        encode_symbol(value, output)?;
    }

    match &node.content {
        NodeContent::Empty => output.push(0),
        NodeContent::Bytes(payload) => {
            let len = u32::try_from(payload.len()).map_err(|_| BinaryNodeError::PayloadTooLarge)?;
            output.push(1);
            output.extend_from_slice(&len.to_be_bytes());
            output.extend_from_slice(payload.as_ref());
        }
        NodeContent::Nodes(nodes) => {
            let count = u16::try_from(nodes.len()).map_err(|_| BinaryNodeError::TooManyChildren)?;
            output.push(2);
            output.extend_from_slice(&count.to_be_bytes());
            for node in nodes {
                encode_node(node, output)?;
            }
        }
    }

    Ok(())
}

fn encode_symbol(symbol: &str, output: &mut Vec<u8>) -> Result<(), BinaryNodeError> {
    if let Some(index) = token_for_symbol(symbol) {
        output.push(1);
        output.push(index);
        return Ok(());
    }

    let symbol_bytes = symbol.as_bytes();
    let len = u16::try_from(symbol_bytes.len()).map_err(|_| BinaryNodeError::SymbolTooLong)?;

    output.push(2);
    output.extend_from_slice(&len.to_be_bytes());
    output.extend_from_slice(symbol_bytes);
    Ok(())
}

fn token_for_symbol(symbol: &str) -> Option<u8> {
    SINGLE_BYTE_TOKENS
        .iter()
        .position(|token| *token == symbol)
        .and_then(|index| u8::try_from(index).ok())
}
