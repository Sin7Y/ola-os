use std::net::SocketAddr;

use tokio::net::TcpListener;

pub struct OffChainVerifierServer {}

impl OffChainVerifierServer {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(&mut self, listen_addr: SocketAddr) {
        let listener = TcpListener::bind(listen_addr).await.unwrap();
        loop {
            let (stream, addr) = listener.accept().await.unwrap();
            olaos_logs::info!("New offchian verifier connection from {:?}", addr);
        }
    }
}
