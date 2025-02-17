#[derive(Debug, Clone)]
pub struct Credentials {
    api_key: String,
    secret_key: String,
}

impl Credentials {
    pub fn new(api_key: String, secret_key: String) -> Self {
        Self {
            api_key,
            secret_key,
        }
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn secret_key(&self) -> &str {
        &self.secret_key
    }
}
