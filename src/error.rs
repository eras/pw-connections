use std::io;
use thiserror::Error;

use crate::config;

#[derive(Error, Debug)]
pub(crate) enum Error {
    #[error(transparent)]
    IOError(#[from] io::Error),

    #[error(transparent)]
    ConfigError(#[from] config::Error),

    #[error(transparent)]
    PipewireError(#[from] pipewire::Error),
}
