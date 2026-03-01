use std::{
    collections::HashMap,
    io::Read,
    sync::OnceLock,
};

use bytes::Bytes;
use flate2::read::ZlibDecoder;

use crate::wa::{
    error::BinaryNodeError,
    wabinary_tokens::{DOUBLE_BYTE_TOKENS, SINGLE_BYTE_TOKENS},
};

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

/// Decodes a synthetic payload into a [`BinaryNode`].
pub fn decode(input: &[u8]) -> Result<BinaryNode, BinaryNodeError> {
    decode_synthetic(input)
}

/// Encodes a [`BinaryNode`] into synthetic binary format.
pub fn encode(node: &BinaryNode) -> Result<Vec<u8>, BinaryNodeError> {
    encode_synthetic(node)
}

/// Decodes payload in the local synthetic fixture format.
pub fn decode_synthetic(input: &[u8]) -> Result<BinaryNode, BinaryNodeError> {
    let mut decoder = SyntheticDecoder::new(input);
    let node = decoder.decode_node()?;

    if !decoder.is_eof() {
        return Err(BinaryNodeError::TrailingBytes);
    }

    Ok(node)
}

/// Encodes payload in the local synthetic fixture format.
pub fn encode_synthetic(node: &BinaryNode) -> Result<Vec<u8>, BinaryNodeError> {
    let mut output = Vec::new();
    encode_synthetic_node(node, &mut output)?;
    Ok(output)
}

/// Decodes a WAWeb (Baileys-compatible) binary payload.
///
/// Input must be a decrypted noise frame and still contain the leading
/// compression byte used by WABinary (`0x00` for plain, bit `0x02` for zlib).
pub fn decode_real(input: &[u8]) -> Result<BinaryNode, BinaryNodeError> {
    let decompressed = decompress_if_required(input)?;
    let mut decoder = RealDecoder::new(&decompressed);
    let node = decoder.decode_node()?;
    if !decoder.is_eof() {
        return Err(BinaryNodeError::TrailingBytes);
    }

    Ok(node)
}

/// Encodes a WAWeb (Baileys-compatible) payload with no compression.
pub fn encode_real(node: &BinaryNode) -> Result<Vec<u8>, BinaryNodeError> {
    let mut encoder = RealEncoder::new();
    encoder.push_byte(0);
    encoder.encode_node(node)?;
    Ok(encoder.into_inner())
}

const SYNTHETIC_SINGLE_BYTE_TOKENS: [&str; 256] = build_synthetic_single_byte_tokens();

const fn build_synthetic_single_byte_tokens() -> [&'static str; 256] {
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

struct SyntheticDecoder<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> SyntheticDecoder<'a> {
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
                let value = SYNTHETIC_SINGLE_BYTE_TOKENS[token];
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

fn encode_synthetic_node(node: &BinaryNode, output: &mut Vec<u8>) -> Result<(), BinaryNodeError> {
    encode_synthetic_symbol(&node.tag, output)?;

    let attrs_len = u16::try_from(node.attrs.len()).map_err(|_| BinaryNodeError::TooManyAttributes)?;
    output.extend_from_slice(&attrs_len.to_be_bytes());

    let mut keys: Vec<&String> = node.attrs.keys().collect();
    keys.sort_unstable();

    for key in keys {
        let value = node
            .attrs
            .get(key)
            .ok_or(BinaryNodeError::AttributeLookupFailed)?;
        encode_synthetic_symbol(key, output)?;
        encode_synthetic_symbol(value, output)?;
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
                encode_synthetic_node(node, output)?;
            }
        }
    }

    Ok(())
}

fn encode_synthetic_symbol(symbol: &str, output: &mut Vec<u8>) -> Result<(), BinaryNodeError> {
    if let Some(index) = synthetic_token_for_symbol(symbol) {
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

fn synthetic_token_for_symbol(symbol: &str) -> Option<u8> {
    SYNTHETIC_SINGLE_BYTE_TOKENS
        .iter()
        .position(|token| *token == symbol)
        .and_then(|index| u8::try_from(index).ok())
}

#[derive(Debug, Clone, Copy)]
struct WaTokenIndex {
    dict: Option<u8>,
    index: u8,
}

fn wa_token_map() -> &'static HashMap<&'static str, WaTokenIndex> {
    static TOKEN_MAP: OnceLock<HashMap<&'static str, WaTokenIndex>> = OnceLock::new();
    TOKEN_MAP.get_or_init(|| {
        let mut map = HashMap::new();

        for (index, token) in SINGLE_BYTE_TOKENS.iter().enumerate() {
            if let Ok(index) = u8::try_from(index) {
                map.insert(*token, WaTokenIndex { dict: None, index });
            }
        }

        for (dict, row) in DOUBLE_BYTE_TOKENS.iter().enumerate() {
            if let Ok(dict) = u8::try_from(dict) {
                for (index, token) in row.iter().enumerate() {
                    if let Ok(index) = u8::try_from(index) {
                        map.insert(*token, WaTokenIndex {
                            dict: Some(dict),
                            index,
                        });
                    }
                }
            }
        }

        map
    })
}

#[derive(Debug, Clone, Copy)]
enum WaTag {
    ListEmpty = 0,
    Dictionary0 = 236,
    Dictionary1 = 237,
    Dictionary2 = 238,
    Dictionary3 = 239,
    InteropJid = 245,
    FbJid = 246,
    AdJid = 247,
    List8 = 248,
    List16 = 249,
    JidPair = 250,
    Hex8 = 251,
    Binary8 = 252,
    Binary20 = 253,
    Binary32 = 254,
    Nibble8 = 255,
}

impl WaTag {
    fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::ListEmpty,
            236 => Self::Dictionary0,
            237 => Self::Dictionary1,
            238 => Self::Dictionary2,
            239 => Self::Dictionary3,
            245 => Self::InteropJid,
            246 => Self::FbJid,
            247 => Self::AdJid,
            248 => Self::List8,
            249 => Self::List16,
            250 => Self::JidPair,
            251 => Self::Hex8,
            252 => Self::Binary8,
            253 => Self::Binary20,
            254 => Self::Binary32,
            255 => Self::Nibble8,
            _ => return None,
        })
    }
}

struct RealDecoder<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> RealDecoder<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }

    fn is_eof(&self) -> bool {
        self.position == self.input.len()
    }

    fn decode_node(&mut self) -> Result<BinaryNode, BinaryNodeError> {
        let list_tag = self.read_byte()?;
        let list_size = self.read_list_size(list_tag)?;
        let header_tag = self.read_byte()?;
        let header = self.read_string(header_tag)?;

        if list_size == 0 || header.is_empty() {
            return Err(BinaryNodeError::InvalidFrame("invalid wa node"));
        }

        let attrs_len = (list_size - 1) >> 1;
        let mut attrs = HashMap::with_capacity(attrs_len);

        for _ in 0..attrs_len {
            let key_tag = self.read_byte()?;
            let key = self.read_string(key_tag)?;
            let value_tag = self.read_byte()?;
            let value = self.read_string(value_tag)?;
            attrs.insert(key, value);
        }

        let content = if list_size % 2 == 0 {
            let tag = self.read_byte()?;
            if self.is_list_tag(tag) {
                NodeContent::Nodes(self.read_list(tag)?)
            } else {
                let bytes = match WaTag::from_u8(tag) {
                    Some(WaTag::Binary8) => {
                        let len = self.read_byte()? as usize;
                        self.read_bytes(len)?.to_vec()
                    }
                    Some(WaTag::Binary20) => {
                        let len = self.read_int20()?;
                        self.read_bytes(len)?.to_vec()
                    }
                    Some(WaTag::Binary32) => {
                        let len = self.read_int(4)?;
                        self.read_bytes(len)?.to_vec()
                    }
                    _ => self.read_string(tag)?.into_bytes(),
                };
                NodeContent::Bytes(Bytes::from(bytes))
            }
        } else {
            NodeContent::Empty
        };

        Ok(BinaryNode {
            tag: header,
            attrs,
            content,
        })
    }

    fn is_list_tag(&self, tag: u8) -> bool {
        matches!(WaTag::from_u8(tag), Some(WaTag::ListEmpty | WaTag::List8 | WaTag::List16))
    }

    fn read_list(&mut self, tag: u8) -> Result<Vec<BinaryNode>, BinaryNodeError> {
        let size = self.read_list_size(tag)?;
        let mut out = Vec::with_capacity(size);
        for _ in 0..size {
            out.push(self.decode_node()?);
        }
        Ok(out)
    }

    fn read_list_size(&mut self, tag: u8) -> Result<usize, BinaryNodeError> {
        match WaTag::from_u8(tag) {
            Some(WaTag::ListEmpty) => Ok(0),
            Some(WaTag::List8) => Ok(self.read_byte()? as usize),
            Some(WaTag::List16) => Ok(self.read_int(2)?),
            _ => Err(BinaryNodeError::InvalidListTag(tag)),
        }
    }

    fn read_string(&mut self, tag: u8) -> Result<String, BinaryNodeError> {
        if tag >= 1 && (tag as usize) < SINGLE_BYTE_TOKENS.len() {
            return Ok(SINGLE_BYTE_TOKENS[tag as usize].to_owned());
        }

        match WaTag::from_u8(tag) {
            Some(WaTag::Dictionary0 | WaTag::Dictionary1 | WaTag::Dictionary2 | WaTag::Dictionary3) => {
                let dict_index = tag - WaTag::Dictionary0 as u8;
                let index = self.read_byte()?;
                let dict = DOUBLE_BYTE_TOKENS
                    .get(dict_index as usize)
                    .ok_or(BinaryNodeError::UnknownTokenDictionary(dict_index))?;
                let value = dict
                    .get(index as usize)
                    .ok_or(BinaryNodeError::UnknownDoubleToken {
                        dict: dict_index,
                        index,
                    })?;
                Ok((*value).to_owned())
            }
            Some(WaTag::ListEmpty) => Ok(String::new()),
            Some(WaTag::Binary8) => {
                let len = self.read_byte()? as usize;
                let value = self.read_bytes(len)?;
                String::from_utf8(value.to_vec()).map_err(|_| BinaryNodeError::InvalidUtf8)
            }
            Some(WaTag::Binary20) => {
                let len = self.read_int20()?;
                let value = self.read_bytes(len)?;
                String::from_utf8(value.to_vec()).map_err(|_| BinaryNodeError::InvalidUtf8)
            }
            Some(WaTag::Binary32) => {
                let len = self.read_int(4)?;
                let value = self.read_bytes(len)?;
                String::from_utf8(value.to_vec()).map_err(|_| BinaryNodeError::InvalidUtf8)
            }
            Some(WaTag::JidPair) => self.read_jid_pair(),
            Some(WaTag::AdJid) => self.read_ad_jid(),
            Some(WaTag::FbJid) => self.read_fb_jid(),
            Some(WaTag::InteropJid) => self.read_interop_jid(),
            Some(WaTag::Hex8 | WaTag::Nibble8) => self.read_packed_8(tag),
            _ => Err(BinaryNodeError::InvalidSymbolType(tag)),
        }
    }

    fn read_packed_8(&mut self, tag: u8) -> Result<String, BinaryNodeError> {
        let start = self.read_byte()?;
        let mut out = Vec::new();

        for _ in 0..(start & 0x7F) {
            let cur = self.read_byte()?;
            out.push(unpack_byte(tag, (cur & 0xF0) >> 4)?);
            out.push(unpack_byte(tag, cur & 0x0F)?);
        }

        if start >> 7 != 0 {
            let _ = out.pop();
        }

        String::from_utf8(out).map_err(|_| BinaryNodeError::InvalidUtf8)
    }

    fn read_jid_pair(&mut self) -> Result<String, BinaryNodeError> {
        let user_tag = self.read_byte()?;
        let user = self.read_string(user_tag)?;
        let server_tag = self.read_byte()?;
        let server = self.read_string(server_tag)?;
        if server.is_empty() {
            return Err(BinaryNodeError::InvalidFrame("invalid jid pair"));
        }

        if user.is_empty() {
            Ok(format!("@{server}"))
        } else {
            Ok(format!("{user}@{server}"))
        }
    }

    fn read_ad_jid(&mut self) -> Result<String, BinaryNodeError> {
        let domain_type = self.read_byte()?;
        let device = self.read_byte()?;
        let user_tag = self.read_byte()?;
        let user = self.read_string(user_tag)?;

        let server = match domain_type {
            1 => "lid",
            128 => "hosted",
            129 => "hosted.lid",
            _ => "s.whatsapp.net",
        };

        Ok(jid_encode(&user, server, if device > 0 { Some(device as u16) } else { None }))
    }

    fn read_fb_jid(&mut self) -> Result<String, BinaryNodeError> {
        let user_tag = self.read_byte()?;
        let user = self.read_string(user_tag)?;
        let device = self.read_int(2)?;
        let server_tag = self.read_byte()?;
        let server = self.read_string(server_tag)?;
        Ok(format!("{user}:{device}@{server}"))
    }

    fn read_interop_jid(&mut self) -> Result<String, BinaryNodeError> {
        let user_tag = self.read_byte()?;
        let user = self.read_string(user_tag)?;
        let device = self.read_int(2)?;
        let integrator = self.read_int(2)?;

        let before = self.position;
        let server = match self.read_byte().and_then(|value| self.read_string(value)) {
            Ok(value) => value,
            Err(_) => {
                self.position = before;
                "interop".to_owned()
            }
        };

        Ok(format!("{integrator}-{user}:{device}@{server}"))
    }

    fn read_byte(&mut self) -> Result<u8, BinaryNodeError> {
        let value = self
            .input
            .get(self.position)
            .copied()
            .ok_or(BinaryNodeError::UnexpectedEof)?;
        self.position += 1;
        Ok(value)
    }

    fn read_int(&mut self, n: usize) -> Result<usize, BinaryNodeError> {
        let mut value = 0usize;
        for _ in 0..n {
            value = (value << 8) | self.read_byte()? as usize;
        }
        Ok(value)
    }

    fn read_int20(&mut self) -> Result<usize, BinaryNodeError> {
        let a = (self.read_byte()? as usize) & 0x0F;
        let b = self.read_byte()? as usize;
        let c = self.read_byte()? as usize;
        Ok((a << 16) | (b << 8) | c)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], BinaryNodeError> {
        let end = self
            .position
            .checked_add(n)
            .ok_or(BinaryNodeError::UnexpectedEof)?;
        if end > self.input.len() {
            return Err(BinaryNodeError::UnexpectedEof);
        }

        let value = &self.input[self.position..end];
        self.position = end;
        Ok(value)
    }
}

struct RealEncoder {
    out: Vec<u8>,
}

impl RealEncoder {
    fn new() -> Self {
        Self { out: Vec::new() }
    }

    fn into_inner(self) -> Vec<u8> {
        self.out
    }

    fn push_byte(&mut self, value: u8) {
        self.out.push(value);
    }

    fn push_bytes(&mut self, values: &[u8]) {
        self.out.extend_from_slice(values);
    }

    fn push_int(&mut self, value: usize, n: usize) {
        for i in (0..n).rev() {
            self.out.push(((value >> (i * 8)) & 0xFF) as u8);
        }
    }

    fn encode_node(&mut self, node: &BinaryNode) -> Result<(), BinaryNodeError> {
        if node.tag.is_empty() {
            return Err(BinaryNodeError::InvalidFrame("node tag cannot be empty"));
        }

        let mut attrs: Vec<(&String, &String)> = node.attrs.iter().collect();
        attrs.sort_by(|(a, _), (b, _)| a.cmp(b));

        let content_present = !matches!(node.content, NodeContent::Empty);
        let list_size = (attrs.len() * 2) + 1 + usize::from(content_present);

        self.write_list_start(list_size)?;
        self.write_string(&node.tag)?;

        for (key, value) in attrs {
            self.write_string(key)?;
            self.write_string(value)?;
        }

        match &node.content {
            NodeContent::Empty => {}
            NodeContent::Bytes(bytes) => {
                self.write_byte_length(bytes.len())?;
                self.push_bytes(bytes.as_ref());
            }
            NodeContent::Nodes(nodes) => {
                self.write_list_start(nodes.len())?;
                for child in nodes {
                    self.encode_node(child)?;
                }
            }
        }

        Ok(())
    }

    fn write_list_start(&mut self, size: usize) -> Result<(), BinaryNodeError> {
        if size == 0 {
            self.push_byte(WaTag::ListEmpty as u8);
            return Ok(());
        }

        if size < 256 {
            self.push_byte(WaTag::List8 as u8);
            self.push_byte(size as u8);
            return Ok(());
        }

        let size_u16 = u16::try_from(size).map_err(|_| BinaryNodeError::PayloadTooLarge)?;
        self.push_byte(WaTag::List16 as u8);
        self.push_bytes(&size_u16.to_be_bytes());
        Ok(())
    }

    fn write_byte_length(&mut self, length: usize) -> Result<(), BinaryNodeError> {
        if length >= (1usize << 32) {
            return Err(BinaryNodeError::PayloadTooLarge);
        }

        if length >= (1usize << 20) {
            self.push_byte(WaTag::Binary32 as u8);
            self.push_int(length, 4);
        } else if length >= 256 {
            self.push_byte(WaTag::Binary20 as u8);
            self.push_byte(((length >> 16) & 0x0F) as u8);
            self.push_byte(((length >> 8) & 0xFF) as u8);
            self.push_byte((length & 0xFF) as u8);
        } else {
            self.push_byte(WaTag::Binary8 as u8);
            self.push_byte(length as u8);
        }

        Ok(())
    }

    fn write_string(&mut self, value: &str) -> Result<(), BinaryNodeError> {
        if value.is_empty() {
            self.write_string_raw(value)?;
            return Ok(());
        }

        if let Some(token) = wa_token_map().get(value) {
            if let Some(dict) = token.dict {
                self.push_byte(WaTag::Dictionary0 as u8 + dict);
            }
            self.push_byte(token.index);
            return Ok(());
        }

        if is_nibble(value) {
            self.write_packed_bytes(value, WaTag::Nibble8 as u8)?;
            return Ok(());
        }

        if is_hex(value) {
            self.write_packed_bytes(value, WaTag::Hex8 as u8)?;
            return Ok(());
        }

        if let Some(jid) = jid_decode(value) {
            self.write_jid(jid)?;
            return Ok(());
        }

        self.write_string_raw(value)
    }

    fn write_string_raw(&mut self, value: &str) -> Result<(), BinaryNodeError> {
        let bytes = value.as_bytes();
        self.write_byte_length(bytes.len())?;
        self.push_bytes(bytes);
        Ok(())
    }

    fn write_jid(&mut self, jid: DecodedJid) -> Result<(), BinaryNodeError> {
        if let Some(device) = jid.device {
            self.push_byte(WaTag::AdJid as u8);
            self.push_byte(jid.domain_type);
            self.push_byte(u8::try_from(device).map_err(|_| BinaryNodeError::InvalidFrame("jid device id overflow"))?);
            self.write_string(&jid.user)?;
            return Ok(());
        }

        self.push_byte(WaTag::JidPair as u8);
        if jid.user.is_empty() {
            self.push_byte(WaTag::ListEmpty as u8);
        } else {
            self.write_string(&jid.user)?;
        }

        self.write_string(&jid.server)?;
        Ok(())
    }

    fn write_packed_bytes(&mut self, value: &str, tag: u8) -> Result<(), BinaryNodeError> {
        if value.len() > 127 {
            return Err(BinaryNodeError::SymbolTooLong);
        }

        self.push_byte(tag);

        let mut rounded = value.len().div_ceil(2) as u8;
        if value.len() % 2 != 0 {
            rounded |= 0x80;
        }
        self.push_byte(rounded);

        let chars: Vec<char> = value.chars().collect();
        for chunk in chars.chunks(2) {
            let left = pack_char(chunk[0], tag)?;
            let right = if chunk.len() == 2 {
                pack_char(chunk[1], tag)?
            } else {
                0x0F
            };
            self.push_byte((left << 4) | right);
        }

        Ok(())
    }
}

fn decompress_if_required(input: &[u8]) -> Result<Vec<u8>, BinaryNodeError> {
    let Some(first) = input.first().copied() else {
        return Err(BinaryNodeError::InvalidCompressedNode);
    };

    if first & 0x02 != 0 {
        let mut decoder = ZlibDecoder::new(&input[1..]);
        let mut out = Vec::new();
        decoder
            .read_to_end(&mut out)
            .map_err(|error| BinaryNodeError::InflateFailed(error.to_string()))?;
        return Ok(out);
    }

    if input.len() < 2 {
        return Err(BinaryNodeError::InvalidCompressedNode);
    }

    Ok(input[1..].to_vec())
}

fn unpack_hex(value: u8) -> Result<u8, BinaryNodeError> {
    match value {
        0..=9 => Ok(b'0' + value),
        10..=15 => Ok(b'A' + (value - 10)),
        _ => Err(BinaryNodeError::InvalidPackedChar(value)),
    }
}

fn unpack_nibble(value: u8) -> Result<u8, BinaryNodeError> {
    match value {
        0..=9 => Ok(b'0' + value),
        10 => Ok(b'-'),
        11 => Ok(b'.'),
        15 => Ok(0),
        _ => Err(BinaryNodeError::InvalidPackedChar(value)),
    }
}

fn unpack_byte(tag: u8, value: u8) -> Result<u8, BinaryNodeError> {
    match WaTag::from_u8(tag) {
        Some(WaTag::Nibble8) => unpack_nibble(value),
        Some(WaTag::Hex8) => unpack_hex(value),
        _ => Err(BinaryNodeError::InvalidListTag(tag)),
    }
}

fn pack_char(value: char, tag: u8) -> Result<u8, BinaryNodeError> {
    match WaTag::from_u8(tag) {
        Some(WaTag::Nibble8) => match value {
            '0'..='9' => Ok(value as u8 - b'0'),
            '-' => Ok(10),
            '.' => Ok(11),
            '\0' => Ok(15),
            _ => Err(BinaryNodeError::InvalidPackedChar(value as u8)),
        },
        Some(WaTag::Hex8) => match value {
            '0'..='9' => Ok(value as u8 - b'0'),
            'A'..='F' => Ok(10 + (value as u8 - b'A')),
            'a'..='f' => Ok(10 + (value as u8 - b'a')),
            '\0' => Ok(15),
            _ => Err(BinaryNodeError::InvalidPackedChar(value as u8)),
        },
        _ => Err(BinaryNodeError::InvalidListTag(tag)),
    }
}

fn is_nibble(value: &str) -> bool {
    if value.is_empty() || value.len() > 127 {
        return false;
    }

    value
        .bytes()
        .all(|char| char.is_ascii_digit() || char == b'-' || char == b'.')
}

fn is_hex(value: &str) -> bool {
    if value.is_empty() || value.len() > 127 {
        return false;
    }

    value
        .bytes()
        .all(|char| char.is_ascii_digit() || (b'A'..=b'F').contains(&char))
}

#[derive(Debug, Clone)]
struct DecodedJid {
    user: String,
    server: String,
    device: Option<u16>,
    domain_type: u8,
}

fn jid_decode(value: &str) -> Option<DecodedJid> {
    let (user_part, server) = value.split_once('@')?;
    let (user_agent, device) = if let Some((ua, device)) = user_part.split_once(':') {
        let parsed = device.parse::<u16>().ok()?;
        (ua, Some(parsed))
    } else {
        (user_part, None)
    };

    let (user, agent) = if let Some((user, agent)) = user_agent.split_once('_') {
        (user, Some(agent))
    } else {
        (user_agent, None)
    };

    let domain_type = match server {
        "lid" => 1,
        "hosted" => 128,
        "hosted.lid" => 129,
        _ => agent.and_then(|raw| raw.parse::<u8>().ok()).unwrap_or(0),
    };

    Some(DecodedJid {
        user: user.to_owned(),
        server: server.to_owned(),
        device,
        domain_type,
    })
}

fn jid_encode(user: &str, server: &str, device: Option<u16>) -> String {
    if let Some(device) = device {
        format!("{user}:{device}@{server}")
    } else {
        format!("{user}@{server}")
    }
}
