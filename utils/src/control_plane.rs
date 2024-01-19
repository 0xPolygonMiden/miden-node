use anyhow::{anyhow, Result};
use miden_node_proto::control_plane::api_server::ApiServer;
use miden_node_proto::control_plane::{api_server, ShutdownRequest, ShutdownResponse};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt::Display, future::Future, net::ToSocketAddrs};
use tokio::sync::{oneshot, RwLock};
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::{error, info};

use crate::config::Endpoint;

pub const COMPONENT: &str = "control_plane";

// CONTROL PLANE CONFIG
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct ControlPlaneConfig {
    /// Defines the listening socket.
    pub endpoint: Endpoint,
}

// CONTROL PLANE
// ================================================================================================

type ShutdownMsg = ();

/// Logic to receive administering commands from the user, and dispatch commands to the running
/// server.
#[derive(Debug, Default)]
pub struct ControlPlane {
    /// List of tasks registered to be notified on shutdown.
    ///
    /// The control plane constructor will allocated a vector to store the waiter. Because the
    /// server can be stopped only once, when that happens the vector is taken. If a client tries to
    /// request for a shutdown signal after that, an error is returned instead.
    shutdown: Option<Vec<oneshot::Sender<ShutdownMsg>>>,
}

/// Thin wrapper around [ControlPlane] providing interior mutability.
#[derive(Debug)]
pub struct ControlPlaneServer {
    control_plane: RwLock<ControlPlane>,
}

impl ControlPlaneServer {
    pub fn new(control_plane: ControlPlane) -> Self {
        Self {
            control_plane: RwLock::new(control_plane),
        }
    }
}

#[derive(Debug)]
pub enum ControlPlaneError {
    PlaneAlreadyShutdown,
}

impl Display for ControlPlaneError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            ControlPlaneError::PlaneAlreadyShutdown => write!(f, "Server shutdown started"),
        }
    }
}

impl Error for ControlPlaneError {}

impl ControlPlane {
    pub fn new() -> Self {
        Self {
            shutdown: Some(Vec::new()),
        }
    }

    /// Returns a oneshot channel which will receive a shutdown msg.
    ///
    /// If the control plane has already been closed, returns an error instead.
    pub fn shutdown_waiter(&mut self) -> Result<oneshot::Receiver<ShutdownMsg>, ControlPlaneError> {
        match self.shutdown {
            Some(ref mut shutdown) => {
                let (sender, receiver) = oneshot::channel();
                shutdown.push(sender);
                Ok(receiver)
            },
            _ => Err(ControlPlaneError::PlaneAlreadyShutdown),
        }
    }

    /// Shutdown the server.
    ///
    /// This can be done only once, subsequent calls are noops.
    pub fn shutdown(&mut self) {
        if let Some(waiter) = self.shutdown.take() {
            for sender in waiter {
                if sender.send(()).is_err() {
                    error!(COMPONENT, "Sending shutdown message failed");
                };
            }
        }
    }
}

#[tonic::async_trait]
impl api_server::Api for ControlPlaneServer {
    async fn shutdown(
        &self,
        _request: Request<ShutdownRequest>,
    ) -> Result<Response<ShutdownResponse>, Status> {
        let mut control_plane = self.control_plane.write().await;
        control_plane.shutdown();
        Ok(Response::new(ShutdownResponse {}))
    }
}

// UTILS
// ================================================================================================

/// Creates a server for the control plane.
///
/// This consumed the control plane, since it will be driven by the server. Any additional setup
/// with the contorl plane must be done before creating the server with this function.
pub async fn create_server(
    config: ControlPlaneConfig,
    mut control_plane: ControlPlane,
) -> Result<impl Future<Output = Result<()>>> {
    let shutdown = control_plane.shutdown_waiter()?;
    let endpoint = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = endpoint.to_socket_addrs()?.collect();
    let svc = ApiServer::new(ControlPlaneServer::new(control_plane));

    Ok(async move {
        Server::builder()
            .add_service(svc)
            .serve_with_shutdown(addrs[0], async {
                match shutdown.await {
                    Ok(_) => info!(COMPONENT, "Control plane shutdown"),
                    Err(_) => error!(COMPONENT, "Control plane channel closed"),
                }
            })
            .await
            .map_err(|e| anyhow!("Server failed: {e:?}"))
    })
}
