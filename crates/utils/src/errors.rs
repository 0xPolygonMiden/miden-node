use thiserror::Error;
use tonic::transport::Error as TransportError;

#[derive(Debug, Error)]
pub enum ApiError {
    /// The initialisation of the Api has failed
    #[error("initialisation of the Api has failed: {0}")]
    ApiInitialisationFailed(TransportError),

    /// Serving the Api at a certain address has failed
    #[error("Serving the Api server has failed.")]
    ApiServeFailed(TransportError),

    /// The resolution of the server address has failed
    #[error("Resolution of the server address has failed: {0}")]
    AddressResolutionFailed(String),

    /// Converting the provided `Endpoint` into a socket address has failed
    #[error("Converting the `Endpoint` into a socket address failed: {0}")]
    EndpointToSocketFailed(std::io::Error),

    /// The connection to the database has failed
    #[error("Connection to the database has failed: {0}")]
    DatabaseConnectionFailed(String),
}
