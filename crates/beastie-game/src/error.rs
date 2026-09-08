#[derive(Debug, thiserror::Error)]
pub enum GameError {
    #[error("filesystem: {0}")]
    Filesystem(String),
    #[error("configuration: {0}")]
    Config(String),
    #[error("session: {0}")]
    Session(String),
}

pub type GameResult<T = ()> = Result<T, GameError>;
