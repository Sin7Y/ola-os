use derivative::Derivative;
use serde::{Deserialize, Serialize};

#[derive(Derivative, Serialize, Deserialize)]
#[derivative(Clone, Debug, Default, Hash, PartialEq)]
pub struct GeometryConfig {
    // pub cycles_per_vm_snapshot: u32,
    // pub cycles_per_log_demuxer: u32,
    // pub cycles_per_storage_sorter: u32,
    // pub cycles_per_events_or_l1_messages_sorter: u32,
    // pub cycles_per_ram_permutation: u32,
    // pub cycles_per_code_decommitter_sorter: u32,
    // pub cycles_per_code_decommitter: u32,
    // pub cycles_per_storage_application: u32,
    // pub cycles_per_keccak256_circuit: u32,
    // pub cycles_per_sha256_circuit: u32,
    // pub cycles_per_ecrecover_circuit: u32,

    // pub limit_for_l1_messages_pudata_hasher: u32,
    // pub limit_for_l1_messages_merklizer: u32,
    pub limit_for_initial_writes_pubdata_hasher: u32,
    pub limit_for_repeated_writes_pubdata_hasher: u32,
}

pub const fn get_geometry_config() -> GeometryConfig {
    GeometryConfig {
        // cycles_per_vm_snapshot: 22893,
        // cycles_per_code_decommitter_sorter: 192832,
        // cycles_per_log_demuxer: 101830,
        // cycles_per_storage_sorter: 79603,
        // cycles_per_events_or_l1_messages_sorter: 88765,
        // limit_for_l1_messages_merklizer: 512,
        // cycles_per_ram_permutation: 260102,
        // cycles_per_code_decommitter: 12306,
        // cycles_per_storage_application: 118,
        limit_for_initial_writes_pubdata_hasher: 4765,
        limit_for_repeated_writes_pubdata_hasher: 7564,
        // cycles_per_keccak256_circuit: 2141,
        // cycles_per_sha256_circuit: 11812,
        // cycles_per_ecrecover_circuit: 72,
        // limit_for_l1_messages_pudata_hasher: 512,
    }
}

pub const GEOMETRY_CONFIG: GeometryConfig = get_geometry_config();
