pub enum Environment {
    Local,
    PreAlpha,
    Mainnet,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::PreAlpha => "pre-alpha",
            Environment::Mainnet => "mainnet",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "pre-alpha" => Ok(Self::PreAlpha),
            "mainnet" => Ok(Self::Mainnet),
            other => Err(format!(
                "{} is not a supported environment. Use either `local`、`pre-alpha` or `mainnet`.",
                other
            )),
        }
    }
}
