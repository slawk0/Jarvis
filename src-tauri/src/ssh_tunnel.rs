use crate::app_error::AppError;
use crate::ssh::SshConnection;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// A live SSH local port-forward. Listens on `127.0.0.1:<ephemeral>` and proxies
/// every accepted TCP connection to `target_host:target_port` on the remote side
/// through a `direct-tcpip` SSH channel. Dropping the handle aborts the listener
/// task and tears the tunnel down.
pub struct TunnelHandle {
    pub local_addr: SocketAddr,
    task: JoinHandle<()>,
}

impl Drop for TunnelHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Start a tunnel from a fresh loopback port to `target_host:target_port`.
/// Returns the bound local address so a native driver can connect to it.
pub async fn start_tunnel(
    conn: SshConnection,
    target_host: String,
    target_port: u32,
) -> Result<TunnelHandle, AppError> {
    // Bind only to loopback so the forwarded database port is never exposed
    // to the local network.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| AppError::with_details("TUNNEL_BIND_FAILED", e.to_string()))?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| AppError::with_details("TUNNEL_BIND_FAILED", e.to_string()))?;

    let task = tokio::spawn(async move {
        loop {
            let (mut inbound, _peer) = match listener.accept().await {
                Ok(pair) => pair,
                Err(_) => break,
            };
            let conn = conn.clone();
            let target_host = target_host.clone();
            tokio::spawn(async move {
                let channel = match conn.open_forward_channel(&target_host, target_port).await {
                    Ok(ch) => ch,
                    Err(_) => return,
                };
                let mut stream = channel.into_stream();
                // Pump bytes both ways until either side closes.
                let _ = tokio::io::copy_bidirectional(&mut inbound, &mut stream).await;
            });
        }
    });

    Ok(TunnelHandle { local_addr, task })
}
