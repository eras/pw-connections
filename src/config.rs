use serde_derive::Deserialize;
use std::{fmt, fs, io};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ParseError(ParseError),

    #[error(transparent)]
    YamlError(#[from] serde_yaml::Error),

    // #[error(transparent)]
    // TomlSerError(#[from] toml::ser::Error),
    #[error(transparent)]
    IOError(#[from] io::Error),
    // #[error(transparent)]
    // AtomicIOError(#[from] atomicwrites::Error<io::Error>),
}

#[derive(Error, Debug)]
pub struct ParseError {
    pub filename: String,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to parse {}: {}", self.filename, self.message)
    }
}

#[derive(Debug, Deserialize, Clone, Eq, Hash, PartialOrd, PartialEq)]
pub struct PortName(pub String);

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct NamedLink {
    pub src: PortName,
    pub dst: PortName,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NamedLinks(pub Vec<NamedLink>);

#[derive(Debug, Deserialize)]
pub struct Config {
    pub links: NamedLinks,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            links: NamedLinks(vec![]),
        }
    }
}

impl Config {
    // If no file is found, returns default config instead of error
    pub fn load(filename: &str) -> Result<Config, Error> {
        let contents = match fs::read_to_string(filename) {
            Ok(contents) => contents,
            // Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(error) => return Err(Error::IOError(error)),
        };
        let config = match serde_yaml::from_str(&contents) {
            Ok(contents) => contents,
            Err(error) if error.location().is_some() => {
                return Err(Error::ParseError(ParseError {
                    filename: String::from(filename),
                    message: format!("{}", error),
                }));
            }
            Err(error) => return Err(Error::YamlError(error)),
        };
        println!("Loaded config from {}", filename);
        Ok(config)
    }
}
