use ola_basic_types::Address;
use serde::Deserialize;

use crate::{envy_load, load_config};

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ContractsConfig {
    // pub mailbox_facet_addr: Address,
    // pub executor_facet_addr: Address,
    // pub governance_facet_addr: Address,
    // pub diamond_cut_facet_addr: Address,
    // pub getters_facet_addr: Address,
    // pub verifier_addr: Address,
    // pub diamond_init_addr: Address,
    // pub diamond_upgrade_init_addr: Address,
    // pub diamond_proxy_addr: Address,
    // pub validator_timelock_addr: Address,
    // pub genesis_tx_hash: H256,
    // pub l1_erc20_bridge_proxy_addr: Address,
    // pub l1_erc20_bridge_impl_addr: Address,
    pub l2_erc20_bridge_addr: Address,
    // pub l1_weth_bridge_proxy_addr: Option<Address>,
    // pub l2_weth_bridge_addr: Option<Address>,
    // pub l1_allow_list_addr: Address,
    // pub l2_testnet_paymaster_addr: Option<Address>,
    // pub recursion_scheduler_level_vk_hash: H256,
    // pub recursion_node_level_vk_hash: H256,
    // pub recursion_leaf_level_vk_hash: H256,
    // pub recursion_circuits_set_vks_hash: H256,
    // pub l1_multicall3_addr: Address,
}

impl ContractsConfig {
    pub fn from_env() -> Self {
        envy_load("contracts", "OLAOS_CONTRACTS_")
    }
}

pub fn load_contracts_config() -> Result<ContractsConfig, config::ConfigError> {
    load_config("configuration/contracts", "OLAOS_CONTRACTS")
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ola_basic_types::Address;

    use crate::{contracts::load_contracts_config, utils::tests::EnvMutex};

    use super::ContractsConfig;

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_contracts_config() -> ContractsConfig {
        ContractsConfig {
            l2_erc20_bridge_addr: Address::from_str(
                "0xFC073319977e314F251EAE6ae6bE76B0B3BAeeCFFC073319977e314F251EAAAA",
            )
            .expect("failed to initial l2_erc20_bridge_addr"),
        }
    }

    #[test]
    fn test_load_contracts_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_CONTRACTS_L2_ERC20_BRIDGE_ADDR=0xFC073319977e314F251EAE6ae6bE76B0B3BAeeCFFC073319977e314F251EAAAA
        "#;
        lock.set_env(config);

        let contracts_config = load_contracts_config().expect("failed to load contracts config");
        assert_eq!(contracts_config, default_contracts_config());
    }
}
