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
