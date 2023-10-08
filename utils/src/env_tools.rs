use std::{env, str::FromStr, fmt::Debug};

pub fn get_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|e| {
        panic!("Env var {} missing, {}", name, e)
    })
}

pub fn parse_env<F>(name: &str) -> F 
where
    F: FromStr,
    F::Err: Debug,
{
    get_env(name)
        .parse()
        .unwrap_or_else(|e| {
            panic!("Failed to parse env var {}: {:?}", name, e)
        })
}