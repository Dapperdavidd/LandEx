use std::{env, net::IpAddr};

use thiserror::Error;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_DATABASE_MAX_CONNECTIONS: u32 = 10;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: IpAddr,
    pub port: u16,
    pub database_url: String,
    pub database_max_connections: u32,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            host: read_optional("APP_HOST", DEFAULT_HOST)?.parse()?,
            port: read_optional("APP_PORT", DEFAULT_PORT.to_string())?.parse()?,
            database_url: read_required("DATABASE_URL")?,
            database_max_connections: read_optional(
                "DATABASE_MAX_CONNECTIONS",
                DEFAULT_DATABASE_MAX_CONNECTIONS.to_string(),
            )?
            .parse()?,
        })
    }

    pub fn server_address(&self) -> (IpAddr, u16) {
        (self.host, self.port)
    }
}

fn read_required(key: &'static str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::Missing(key))
}

fn read_optional(key: &'static str, default: impl Into<String>) -> Result<String, ConfigError> {
    match env::var(key) {
        Ok(value) if value.trim().is_empty() => Err(ConfigError::Empty(key)),
        Ok(value) => Ok(value),
        Err(env::VarError::NotPresent) => Ok(default.into()),
        Err(env::VarError::NotUnicode(_)) => Err(ConfigError::InvalidUnicode(key)),
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("required environment variable {0} is missing")]
    Missing(&'static str),
    #[error("environment variable {0} cannot be empty")]
    Empty(&'static str),
    #[error("environment variable {0} contains invalid Unicode")]
    InvalidUnicode(&'static str),
    #[error("invalid IP address: {0}")]
    InvalidIp(#[from] std::net::AddrParseError),
    #[error("invalid integer: {0}")]
    InvalidInteger(#[from] std::num::ParseIntError),
}
