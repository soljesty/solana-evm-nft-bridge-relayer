#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    #[error("Database request creation error: {0}")]
    CreationError(String),

    #[error("Token transfer reverted, check approval to the bridge:")]
    EVMTxError(),

    #[error("Token transfer reverted, check approval to the bridge:")]
    SolanaTxError(),

    #[error("Request already processing: {0}")]
    AlreadyExistingRequest(String),

    #[error("A request with that id doesn't exist: {0}")]
    NoExistingRequest(String),

    #[error("Invalid destination account")]
    InvalidDestinationAccount(),
}
