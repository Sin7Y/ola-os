#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let prover_config = FriProverConfig::from_env().context("FriProverConfig::from_env()")?;
}