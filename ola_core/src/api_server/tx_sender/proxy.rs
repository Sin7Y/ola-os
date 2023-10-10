use std::collections::HashMap;

use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use ola_basic_types::H256;
use ola_types::l2::L2Tx;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct TxProxy {
    tx_cache: RwLock<HashMap<H256, L2Tx>>,
    client: HttpClient,
}

impl TxProxy {
    pub fn new(main_node_url: &str) -> Self {
        let client = HttpClientBuilder::default().build(main_node_url).unwrap();
        Self {
            client,
            tx_cache: RwLock::new(HashMap::new()),
        }
    }
}
