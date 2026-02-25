mod common;

use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use chatwarp_api::wa::transport::WsTransport;
use common::ws_mock::start_single_client_server;

#[tokio::test]
async fn transport_roundtrip_frame_sizes() -> anyhow::Result<()> {
    let sizes = [0_usize, 1, 65_535, 65_536];

    for size in sizes {
        let payload: Vec<u8> = (0..size).map(|idx| (idx % 251) as u8).collect();

        let server = start_single_client_server(|mut ws| async move {
            if let Some(Ok(Message::Binary(binary))) = ws.next().await {
                ws.send(Message::Binary(binary)).await?;
                return Ok(());
            }

            anyhow::bail!("server did not receive expected binary frame")
        })
        .await?;

        let mut transport = WsTransport::connect(&server.url).await?;
        transport.send_frame(&payload).await?;
        let echoed = transport.next_frame().await?;
        assert_eq!(echoed.as_ref(), payload.as_slice());

        server.finish().await?;
    }

    Ok(())
}

#[tokio::test]
async fn transport_responds_to_ping_with_pong() -> anyhow::Result<()> {
    let payload = b"ping-safe-payload".to_vec();
    let framed = frame_payload(&payload);

    let server = start_single_client_server(move |mut ws| async move {
        ws.send(Message::Ping(vec![7, 7, 7].into())).await?;
        ws.send(Message::Binary(framed.into())).await?;

        let next = ws.next().await;
        match next {
            Some(Ok(Message::Pong(bytes))) => {
                assert_eq!(&bytes[..], &[7, 7, 7]);
                Ok(())
            }
            Some(Ok(other)) => anyhow::bail!("expected pong, got {other:?}"),
            Some(Err(err)) => Err(err.into()),
            None => anyhow::bail!("server stream closed before pong"),
        }
    })
    .await?;

    let mut transport = WsTransport::connect(&server.url).await?;
    let decoded = transport.next_frame().await?;
    assert_eq!(decoded.as_ref(), payload.as_slice());

    server.finish().await?;
    Ok(())
}

fn frame_payload(payload: &[u8]) -> Vec<u8> {
    let len = payload.len();
    let mut framed = Vec::with_capacity(3 + len);
    framed.push(((len >> 16) & 0xFF) as u8);
    framed.push(((len >> 8) & 0xFF) as u8);
    framed.push((len & 0xFF) as u8);
    framed.extend_from_slice(payload);
    framed
}
