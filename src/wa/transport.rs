use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::HeaderValue,
    },
};

use crate::wa::error::TransportError;

/// WebSocket transport with WA framing (3-byte length prefix).
pub struct WsTransport {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    frame_buffer: Vec<u8>,
}

impl WsTransport {
    /// Connects to a websocket endpoint.
    pub async fn connect(url: &str) -> Result<Self, TransportError> {
        let (stream, _) = if url.contains("web.whatsapp.com") {
            let mut request = url.into_client_request().map_err(TransportError::Connect)?;
            request.headers_mut().insert(
                "Origin",
                HeaderValue::from_static("https://web.whatsapp.com"),
            );
            connect_async(request).await.map_err(TransportError::Connect)?
        } else {
            connect_async(url).await.map_err(TransportError::Connect)?
        };

        Ok(Self {
            stream,
            frame_buffer: Vec::new(),
        })
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
                    if looks_like_raw_close_frame(&data) {
                        tracing::warn!(
                            close_code = decode_close_code(&data),
                            "received raw websocket close frame bytes in binary payload"
                        );
                        return Err(TransportError::Closed);
                    }
                    // Flush queued control-frame responses (e.g. auto-pong) before returning.
                    self.stream.flush().await?;
                    return Ok(Bytes::copy_from_slice(&data));
                }
                Message::Close(frame) => {
                    tracing::warn!(?frame, "websocket peer closed connection");
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

fn looks_like_raw_close_frame(data: &[u8]) -> bool {
    data.len() == 4 && data[0] == 0x88 && data[1] == 0x02
}

fn decode_close_code(data: &[u8]) -> u16 {
    if data.len() < 4 {
        return 0;
    }
    u16::from_be_bytes([data[2], data[3]])
}
