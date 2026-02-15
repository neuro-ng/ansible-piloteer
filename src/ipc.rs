use anyhow::Result;
use opentelemetry::trace::Span;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream}; // [NEW]

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Handshake {
        token: Option<String>,
    },
    TaskStart {
        name: String,
        task_vars: serde_json::Value,
        facts: Option<serde_json::Value>,
    },
    PlayStart {
        name: String,
        host_pattern: String,
    },
    TaskFail {
        name: String,
        result: serde_json::Value,
        facts: Option<serde_json::Value>,
    },
    TaskResult {
        name: String,
        host: String,
        changed: bool,
        failed: bool, // Track if it eventually failed or was recovered
        verbose_result: Option<crate::execution::ExecutionDetails>,
    },
    TaskUnreachable {
        name: String,
        host: String,
        error: String,
        result: serde_json::Value,
    },
    Proceed,
    Retry,
    ModifyVar {
        key: String,
        value: serde_json::Value,
    },
    AiAnalysis {
        task: String,
        analysis: crate::ai::Analysis,
    },
    Continue,
    PlayRecap {
        stats: serde_json::Value,
    },
    ClientDisconnected, // [NEW] Phase 3: Connection Handling
}

pub enum Listener {
    Unix(UnixListener),
    Tcp(TcpListener),
}

pub struct IpcServer {
    listener: Listener,
}

impl IpcServer {
    pub async fn new<P: AsRef<Path>>(socket_path: P, bind_addr: Option<&str>) -> Result<Self> {
        let listener = if let Some(addr) = bind_addr {
            let tcp = TcpListener::bind(addr).await?;
            println!("IpcServer listening on TCP {}", addr);
            Listener::Tcp(tcp)
        } else {
            if socket_path.as_ref().exists() {
                tokio::fs::remove_file(&socket_path).await?;
            }
            let unix = UnixListener::bind(socket_path)?;
            Listener::Unix(unix)
        };

        Ok(Self { listener })
    }

    pub async fn accept(&self) -> Result<IpcConnection> {
        match &self.listener {
            Listener::Unix(l) => {
                let (stream, _) = l.accept().await?;
                Ok(IpcConnection::new(ConnectionStream::Unix(stream)))
            }
            Listener::Tcp(l) => {
                let (stream, _) = l.accept().await?;
                Ok(IpcConnection::new(ConnectionStream::Tcp(stream)))
            }
        }
    }
}

pub enum ConnectionStream {
    Unix(UnixStream),
    Tcp(TcpStream),
}

impl AsyncRead for ConnectionStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            ConnectionStream::Unix(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            ConnectionStream::Tcp(s) => std::pin::Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for ConnectionStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match self.get_mut() {
            ConnectionStream::Unix(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            ConnectionStream::Tcp(s) => std::pin::Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            ConnectionStream::Unix(s) => std::pin::Pin::new(s).poll_flush(cx),
            ConnectionStream::Tcp(s) => std::pin::Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            ConnectionStream::Unix(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            ConnectionStream::Tcp(s) => std::pin::Pin::new(s).poll_shutdown(cx),
        }
    }
}

pub struct IpcConnection {
    stream: ConnectionStream,
}

impl IpcConnection {
    pub fn new(stream: ConnectionStream) -> Self {
        Self { stream }
    }

    pub async fn send(&mut self, msg: &Message) -> Result<()> {
        let mut span =
            crate::telemetry::start_span("ipc.send", opentelemetry::trace::SpanKind::Producer);

        let result: Result<()> = async {
            let mut json = serde_json::to_string(msg)?;
            json.push('\n');
            self.stream.write_all(json.as_bytes()).await?;
            self.stream.flush().await?;
            Ok(())
        }
        .await;

        if let Err(e) = &result {
            crate::telemetry::record_error_on_span(&mut span, &e.to_string());
        }
        span.end();
        result
    }

    pub async fn receive(&mut self) -> Result<Option<Message>> {
        let mut span =
            crate::telemetry::start_span("ipc.receive", opentelemetry::trace::SpanKind::Consumer);

        let result: Result<Option<Message>> = async {
            let mut reader = BufReader::new(&mut self.stream);
            let mut line = String::new();
            if reader.read_line(&mut line).await? == 0 {
                return Ok(None);
            }
            let msg = serde_json::from_str(&line)?;
            Ok(Some(msg))
        }
        .await;

        if let Err(e) = &result {
            crate::telemetry::record_error_on_span(&mut span, &e.to_string());
        }
        span.end();
        result
    }
}
