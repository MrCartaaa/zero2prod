use secrecy::zeroize::Zeroize;
use secrecy::{CloneableSecret, SecretBox};
use serde::{Deserialize, Deserializer};

#[derive(Clone, Deserialize, Debug)]
pub struct AuthToken {
    pub token: String,
}

impl AuthToken {
    pub fn new(token: String) -> SecretAuthToken {
        SecretBox::new(Box::new(AuthToken { token }))
    }

    pub fn deserialize_from_str<'de, D>(deserializer: D) -> Result<SecretAuthToken, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer) {
            Ok(s) => Ok(AuthToken::new(s)),
            Err(_) => Err(serde::de::Error::custom("bad token")),
        }
    }
}

impl Zeroize for AuthToken {
    fn zeroize(&mut self) {
        self.token.zeroize();
    }
}

impl CloneableSecret for AuthToken {}

pub type SecretAuthToken = SecretBox<AuthToken>;
