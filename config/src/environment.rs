pub enum Environment {
    Local,
    Testnet1,
    Mainnet,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Testnet1 => "testnet1",
            Environment::Mainnet => "mainnet",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "testnet1" => Ok(Self::Testnet1),
            "mainnet" => Ok(Self::Mainnet),
            other => Err(format!(
                "{} is not a supported environment. Use either `local`、`testnet1` or `mainnet`.",
                other
            )),
        }
    }
}
