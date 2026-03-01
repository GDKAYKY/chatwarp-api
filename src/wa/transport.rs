use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{HeaderName, HeaderValue, Response},
    },
};

use crate::wa::error::TransportError;

/// Additional websocket request headers used during connect.
#[derive(Debug, Clone, Default)]
pub struct WsConnectOptions {
    pub origin: Option<String>,
    pub user_agent: Option<String>,
    pub subprotocol: Option<String>,
    pub headers: Vec<(String, String)>,
}

/// Metadata captured from the websocket HTTP upgrade response.
#[derive(Debug, Clone, Default)]
pub struct WsUpgradeMetadata {
    pub status: u16,
    pub headers: Vec<(String, String)>,
}

/// WebSocket transport with WA framing (3-byte length prefix).
pub struct WsTransport {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    frame_buffer: Vec<u8>,
    upgrade: WsUpgradeMetadata,
}

impl WsTransport {
    /// Connects to a websocket endpoint using default headers.
    pub async fn connect(url: &str) -> Result<Self, TransportError> {
        let mut options = WsConnectOptions::default();
        if url.contains("web.whatsapp.com") {
            options.origin = Some("https://web.whatsapp.com".to_owned());
        }
        Self::connect_with_options(url, options).await
    }

    /// Connects to a websocket endpoint with custom request options.
    pub async fn connect_with_options(url: &str, options: WsConnectOptions) -> Result<Self, TransportError> {
        let mut request = url.into_client_request().map_err(TransportError::Connect)?;

        if let Some(origin) = options.origin {
            let value = HeaderValue::from_str(&origin)
                .map_err(|_| TransportError::InvalidFrame("invalid origin header value"))?;
            request.headers_mut().insert("Origin", value);
        }

        if let Some(user_agent) = options.user_agent {
            let value = HeaderValue::from_str(&user_agent)
                .map_err(|_| TransportError::InvalidFrame("invalid user-agent header value"))?;
            request.headers_mut().insert("User-Agent", value);
        }

        if let Some(subprotocol) = options.subprotocol {
            let value = HeaderValue::from_str(&subprotocol)
                .map_err(|_| TransportError::InvalidFrame("invalid subprotocol header value"))?;
            request.headers_mut().insert("Sec-WebSocket-Protocol", value);
        }

        for (key, value) in options.headers {
            let key = HeaderName::from_bytes(key.as_bytes())
                .map_err(|_| TransportError::InvalidFrame("invalid request header name"))?;
            let value = HeaderValue::from_str(&value)
                .map_err(|_| TransportError::InvalidFrame("invalid request header value"))?;
            request.headers_mut().insert(key, value);
        }

        let (stream, response) = connect_async(request).await.map_err(TransportError::Connect)?;

        Ok(Self {
            stream,
            frame_buffer: Vec::new(),
            upgrade: to_upgrade_metadata(&response),
        })
    }

    /// Returns metadata from the HTTP websocket upgrade response.
    pub fn upgrade_metadata(&self) -> &WsUpgradeMetadata {
        &self.upgrade
    }

    /// Sends a framed payload with a 24-bit big-endian prefix.
    pub async fn send_frame(&mut self, payload: &[u8]) -> Result<(), TransportError> {
        if payload.len() > 0xFF_FF_FF {
            return Err(TransportError::FrameTooLarge);
        }

        let len = payload.len();
        let mut frame = Vec::with_capacity(3 + len);
        frame.push(((len >> 16) & 0xFF) as u8);
        frame.push(((len >> 8) & 0xFF) as u8);
        frame.push((len & 0xFF) as u8);
        frame.extend_from_slice(payload);

        self.stream.send(Message::Binary(frame.into())).await?;
        Ok(())
    }

    /// Sends raw websocket binary payload (caller is responsible for WA framing).
    pub async fn send_raw(&mut self, payload: &[u8]) -> Result<(), TransportError> {
        self.stream.send(Message::Binary(payload.to_vec().into())).await?;
        Ok(())
    }

    /// Reads the next raw websocket binary payload, replying to ping frames automatically.
    pub async fn next_raw_frame(&mut self) -> Result<Bytes, TransportError> {
        loop {
            let message = self
                .stream
                .next()
                .await
                .ok_or(TransportError::Closed)??;

            match message {
                Message::Ping(payload) => {
                    self.stream.send(Message::Pong(payload)).await?;
                }
                Message::Binary(data) => {
                    if let Some(close_code) = decode_raw_close_code(&data) {
                        tracing::warn!(close_code, "received raw websocket close frame bytes in binary payload");
                        return Err(TransportError::ClosedWithCode(close_code));
                    }
                    // Flush queued control-frame responses (e.g. auto-pong) before returning.
                    self.stream.flush().await?;
                    return Ok(Bytes::copy_from_slice(&data));
                }
                Message::Close(frame) => {
                    tracing::warn!(?frame, "websocket peer closed connection");
                    if let Some(code) = frame.map(|close| close.code.into()) {
                        return Err(TransportError::ClosedWithCode(code));
                    }
                    return Err(TransportError::Closed);
                }
                Message::Pong(_) => continue,
                Message::Text(_) => continue,
                _ => continue,
            }
        }
    }

    /// Reads the next framed payload, replying to ping frames automatically.
    pub async fn next_frame(&mut self) -> Result<Bytes, TransportError> {
        loop {
            if let Some(payload) = pop_framed_payload(&mut self.frame_buffer)? {
                return Ok(payload);
            }

            let data = self.next_raw_frame().await?;
            self.frame_buffer.extend_from_slice(&data);
        }
    }
}

fn to_upgrade_metadata(response: &Response<Option<Vec<u8>>>) -> WsUpgradeMetadata {
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            let value = value.to_str().ok().map(str::to_owned).unwrap_or_default();
            (name.as_str().to_owned(), value)
        })
        .collect();

    WsUpgradeMetadata {
        status: response.status().as_u16(),
        headers,
    }
}

fn pop_framed_payload(buffer: &mut Vec<u8>) -> Result<Option<Bytes>, TransportError> {
    if buffer.len() < 3 {
        return Ok(None);
    }

    let expected_len = ((buffer[0] as usize) << 16) | ((buffer[1] as usize) << 8) | buffer[2] as usize;
    let full_frame_len = 3 + expected_len;
    if buffer.len() < full_frame_len {
        return Ok(None);
    }

    let payload = buffer[3..full_frame_len].to_vec();
    buffer.drain(..full_frame_len);
    Ok(Some(Bytes::from(payload)))
}

fn decode_raw_close_code(data: &[u8]) -> Option<u16> {
    if data.len() == 4 && data[0] == 0x88 && data[1] == 0x02 {
        return Some(u16::from_be_bytes([data[2], data[3]]));
    }

    None
}
