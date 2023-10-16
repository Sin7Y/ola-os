use std::collections::HashMap;

use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use ola_types::l2::L2Tx;
use ola_types::H256;
use ola_web3_decl::namespaces::ola::OlaNamespaceClient;
use ola_web3_decl::namespaces::RpcResult;
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

    pub async fn save_tx(&self, tx_hash: H256, tx: L2Tx) {
        self.tx_cache.write().await.insert(tx_hash, tx);
    }

    pub async fn submit_tx(&self, tx: &L2Tx) -> RpcResult<H256> {
        let input_data = tx.common_data.input_data().expect("raw tx is absent");
        let raw_tx = ola_types::Bytes(input_data.to_vec());
        olaos_logs::info!("Proxying tx {}", tx.hash());
        self.client.send_raw_transaction(raw_tx).await
    }

    pub async fn forget_tx(&self, tx_hash: H256) {
        self.tx_cache.write().await.remove(&tx_hash);
    }
}
