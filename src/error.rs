use thiserror::Error;

#[derive(Error, Debug)]
pub enum SimmerError {
    #[error("Failed to get the channel {0}")]
    GetChannel(String),
    #[error(
        "The channel failed to downcast, this is likely a bug in simmer, please file an issue :)"
    )]
    DowncastError,
    #[error("Failed to shutdown registry")]
    Shutdown,
    #[error("Tried to convert to channel {0} but it was {0}")]
    WrongSide(&'static str, &'static str),
}

pub type SimmerResult<T> = Result<T, SimmerError>;
