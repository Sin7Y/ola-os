use crate::{
    errors::ClientError,
    operation::{execute_contract::ExecuteContractBuilder, SyncTransactionHandle},
};
use ola_types::{l2::L2Tx, request::CallRequest, Address, Bytes};
use ola_web3_decl::{
    jsonrpsee::http_client::{HttpClient, HttpClientBuilder},
    namespaces::{eth::EthNamespaceClient, ola::OlaNamespaceClient},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderParams {
    pub chain_id: u16,
    pub http_endpoint: String,
}

impl ProviderParams {
    pub fn new(chain_id: u16, http_endpoint: String) -> Self {
        Self {
            chain_id,
            http_endpoint,
        }
    }

    pub fn local() -> Self {
        Self {
            chain_id: 270,
            http_endpoint: "http://localhost:13000".to_string(),
        }
    }

    pub fn pub_test() -> Self {
        Self {
            chain_id: 270,
            http_endpoint: "https://pubtest-api.ola.org".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtendProvider {
    pub provider: HttpClient,
}

impl ExtendProvider {
    pub fn with_http_client(rpc_address: &str) -> Result<ExtendProvider, ClientError> {
        let provider = HttpClientBuilder::default().build(rpc_address)?;

        Ok(ExtendProvider { provider })
    }

    pub async fn call_transaction(&self, call_request: CallRequest) -> Result<Bytes, ClientError> {
        let ret = self.provider.call_transaction(call_request).await?;
        Ok(ret)
    }
}
