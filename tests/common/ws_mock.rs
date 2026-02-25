use std::future::Future;

use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_tungstenite::{WebSocketStream, accept_async};

pub struct WsTestServer {
    pub url: String,
    task: JoinHandle<anyhow::Result<()>>,
}

impl WsTestServer {
    pub async fn finish(self) -> anyhow::Result<()> {
        self.task.await??;
        Ok(())
    }
}

pub async fn start_single_client_server<H, F>(handler: H) -> anyhow::Result<WsTestServer>
where
    H: FnOnce(WebSocketStream<TcpStream>) -> F + Send + 'static,
    F: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let websocket = accept_async(stream).await?;
        handler(websocket).await
    });

    Ok(WsTestServer {
        url: format!("ws://{addr}"),
        task,
    })
}
