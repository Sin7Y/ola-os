use std::env;

pub fn get_current_pod_name() -> String {
    env::var("OLAOS_POD_NAME").unwrap_or("UNKNOWN_POD".to_owned())
}
