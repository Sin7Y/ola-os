pub enum Environment {
    Local,
    PreAlpha,
    Mainnet,
    AlphaDev,
    Alpha,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::PreAlpha => "pre-alpha",
            Environment::Mainnet => "mainnet",
            Environment::AlphaDev => "alpha-dev",
            Environment::Alpha => "alpha",
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
            "alpha-dev" => Ok(Self::AlphaDev),
            "alpha" => Ok(Self::Alpha),
            other => Err(format!(
                "{} is not a supported environment. Use either `local`、`pre-alpha` or `mainnet`.",
                other
            )),
        }
    }
}
