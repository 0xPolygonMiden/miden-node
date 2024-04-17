#[derive(Debug)]
pub enum ApiError {
    ApiInitialisationFailed(String),
    ApiServeFailed(String),
    AddressResolutionFailed(String),
    EndpointToSocketFailed(String),
    DatabaseConnectionFailed(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
