use std::{fmt, str::FromStr};

use anyhow::{anyhow, Error};

/// Different output formats supported by command line
#[derive(Debug, Clone, Copy)]
pub enum OutputType {
    /// Text output format
    Text,
    /// Json output format
    Json,
}

impl fmt::Display for OutputType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Json => write!(f, "json"),
        }
    }
}

impl FromStr for OutputType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            _ => Err(anyhow!("invalid output type")),
        }
    }
}
