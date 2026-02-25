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

        Ok(Self { stream })
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

    /// Reads the next framed payload, replying to ping frames automatically.
    pub async fn next_frame(&mut self) -> Result<Bytes, TransportError> {
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
                    if data.len() < 3 {
                        return Err(TransportError::InvalidFrame("missing 3-byte prefix"));
                    }

                    let expected_len = ((data[0] as usize) << 16)
                        | ((data[1] as usize) << 8)
                        | data[2] as usize;
                    let payload = &data[3..];

                    if payload.len() != expected_len {
                        return Err(TransportError::InvalidFrame("length prefix mismatch"));
                    }

                    // Flush queued control-frame responses (e.g. auto-pong) before returning.
                    self.stream.flush().await?;
                    return Ok(Bytes::copy_from_slice(payload));
                }
                Message::Close(_) => return Err(TransportError::Closed),
                Message::Pong(_) => continue,
                Message::Text(_) => continue,
                _ => continue,
            }
        }
    }
}
