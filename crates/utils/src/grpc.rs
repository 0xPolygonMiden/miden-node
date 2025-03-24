use std::net::SocketAddr;

use crate::errors::ApiError;

/// A sealed extension trait for [`url::Url`] that adds convenience functions for binding and
/// connecting to the url.
pub trait UrlExt: private::Sealed {
    fn to_socket(&self) -> Result<SocketAddr, ApiError>;
}

impl UrlExt for url::Url {
    fn to_socket(&self) -> Result<SocketAddr, ApiError> {
        self.socket_addrs(|| None)
            .map_err(ApiError::EndpointToSocketFailed)?
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::AddressResolutionFailed(self.to_string()))
    }
}

mod private {
    pub trait Sealed {}
    impl Sealed for url::Url {}
}
