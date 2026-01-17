//! Manager client for communicating with the manager daemon.

use std::io;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
};

use crate::server::instance::InstanceInfo;

use super::{
    MANAGER_HOST, MANAGER_PORT,
    protocol::{ManagerMethod, ManagerRequest, ManagerResponse, ManagerResult},
};

/// Client for communicating with the manager daemon.
pub struct ManagerClient {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    next_id: u64,
}

impl ManagerClient {
    /// Connect to the manager daemon at the default address.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect() -> io::Result<Self> {
        Self::connect_to(MANAGER_HOST, MANAGER_PORT).await
    }

    /// Connect to the manager daemon at a specific address.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect_to(host: &str, port: u16) -> io::Result<Self> {
        let stream = TcpStream::connect((host, port)).await?;
        let (reader, writer) = stream.into_split();

        Ok(Self {
            reader: BufReader::new(reader),
            writer: BufWriter::new(writer),
            next_id: 1,
        })
    }

    /// Send a request and receive a response.
    async fn request(&mut self, method: ManagerMethod) -> io::Result<ManagerResponse> {
        let id = self.next_id;
        self.next_id += 1;

        let request = ManagerRequest::new(id, method);
        let request_json = serde_json::to_string(&request)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Send request
        self.writer.write_all(request_json.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;

        // Read response
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;

        let response: ManagerResponse = serde_json::from_str(line.trim())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(response)
    }

    /// Ping the manager daemon.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn ping(&mut self) -> io::Result<bool> {
        let response = self.request(ManagerMethod::Ping).await?;
        Ok(response.result.is_ok())
    }

    /// List all registered instances.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn list(&mut self) -> io::Result<Vec<InstanceInfo>> {
        let response = self.request(ManagerMethod::List).await?;

        match response.result {
            ManagerResult::Ok(value) => {
                let instances: Vec<InstanceInfo> = serde_json::from_value(value)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Ok(instances)
            }
            ManagerResult::Error { message, .. } => Err(io::Error::other(message)),
        }
    }

    /// Query a specific instance by name.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn query(&mut self, name: &str) -> io::Result<Option<InstanceInfo>> {
        let response = self
            .request(ManagerMethod::Query {
                name: name.to_string(),
            })
            .await?;

        match response.result {
            ManagerResult::Ok(value) => {
                let info: InstanceInfo = serde_json::from_value(value)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Ok(Some(info))
            }
            ManagerResult::Error { code: -32001, .. } => {
                // Not found
                Ok(None)
            }
            ManagerResult::Error { message, .. } => Err(io::Error::other(message)),
        }
    }

    /// Register an instance with the manager.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or instance already exists.
    pub async fn register(&mut self, info: InstanceInfo) -> io::Result<()> {
        let response = self.request(ManagerMethod::Register(info)).await?;

        match response.result {
            ManagerResult::Ok(_) => Ok(()),
            ManagerResult::Error { message, .. } => Err(io::Error::other(message)),
        }
    }

    /// Unregister an instance from the manager.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn unregister(&mut self, name: &str) -> io::Result<()> {
        let response = self
            .request(ManagerMethod::Unregister {
                name: name.to_string(),
            })
            .await?;

        match response.result {
            ManagerResult::Ok(_) => Ok(()),
            ManagerResult::Error { message, .. } => Err(io::Error::other(message)),
        }
    }

    /// Request the manager to shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn shutdown(&mut self) -> io::Result<()> {
        let response = self.request(ManagerMethod::Shutdown).await?;

        match response.result {
            ManagerResult::Ok(_) => Ok(()),
            ManagerResult::Error { message, .. } => Err(io::Error::other(message)),
        }
    }
}

#[cfg(test)]
mod tests {
    // Integration tests would require a running manager
    // Unit tests for serialization are in protocol.rs
}
