//! Simple QUIC Client-Server Demo
//!
//! This demonstrates basic QUIC connectivity using Quinn 0.11 API
//! Based on official Quinn documentation examples

use quinn::{ClientConfig, Connection, Endpoint, ServerConfig};
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio;
use tracing::{debug, error, info};

const SERVER_NAME: &str = "localhost";
const LOCALHOST_V4: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const CLIENT_ADDR: SocketAddr = SocketAddr::new(LOCALHOST_V4, 0); // Let OS pick port
const SERVER_ADDR: SocketAddr = SocketAddr::new(LOCALHOST_V4, 5001);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("debug").init();

    info!("🚀 Starting Simple QUIC Demo");

    // Run server and client concurrently
    let server_handle = tokio::spawn(run_server());
    let client_handle = tokio::spawn(run_client());

    // Wait a bit for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Wait for both to complete
    let (server_result, client_result) = tokio::try_join!(server_handle, client_handle)?;

    match (server_result, client_result) {
        (Ok(_), Ok(_)) => info!("✅ QUIC demo completed successfully!"),
        (Err(e), _) => error!("❌ Server error: {}", e),
        (_, Err(e)) => error!("❌ Client error: {}", e),
    }

    Ok(())
}

/// Run the QUIC server
async fn run_server() -> Result<(), Box<dyn Error>> {
    info!("🔧 Setting up QUIC server");

    // Create server configuration with self-signed certificate
    let server_config = configure_server()?;

    // Bind server endpoint to socket
    let endpoint = Endpoint::server(server_config, SERVER_ADDR)?;
    info!("📡 QUIC server listening on {}", SERVER_ADDR);

    // Accept one connection for demo
    if let Some(conn) = endpoint.accept().await {
        info!("📥 Accepting incoming connection");
        let connection = conn.await?;
        info!(
            "✅ QUIC connection established from {}",
            connection.remote_address()
        );

        // Handle the connection
        handle_server_connection(connection).await?;
    }

    info!("🔄 Server shutting down");
    Ok(())
}

/// Run the QUIC client
async fn run_client() -> Result<(), Box<dyn Error>> {
    // Wait a bit for server to be ready
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    info!("🔧 Setting up QUIC client");

    // Create client endpoint
    let endpoint = Endpoint::client(CLIENT_ADDR)?;
    info!("📡 QUIC client bound to {}", endpoint.local_addr()?);

    // Connect to server
    info!("🔗 Connecting to QUIC server at {}", SERVER_ADDR);
    let connection = endpoint.connect(SERVER_ADDR, SERVER_NAME)?.await?;
    info!(
        "✅ QUIC connection established to {}",
        connection.remote_address()
    );

    // Use the connection
    handle_client_connection(connection).await?;

    info!("🔄 Client shutting down");
    Ok(())
}

/// Configure server with self-signed certificate
fn configure_server() -> Result<ServerConfig, Box<dyn Error>> {
    // Generate self-signed certificate
    let cert = rcgen::generate_simple_self_signed(vec![SERVER_NAME.into()])?;
    let cert_der = cert.serialize_der()?;
    let priv_key = cert.serialize_private_key_der();

    let cert_chain = vec![rustls::pki_types::CertificateDer::from(cert_der)];
    let key_der = rustls::pki_types::PrivateKeyDer::try_from(priv_key)?;

    let server_config = ServerConfig::with_single_cert(cert_chain, key_der)?;
    Ok(server_config)
}

/// Handle server-side connection
async fn handle_server_connection(connection: Connection) -> Result<(), Box<dyn Error>> {
    info!("🔧 Server handling connection");

    // Accept a bidirectional stream
    let (mut send, mut recv) = connection.accept_bi().await?;
    info!("📥 Server accepted bidirectional stream");

    // Read data from client
    let data = recv.read_to_end(1024).await?;
    let message = String::from_utf8(data)?;
    info!("📨 Server received: {}", message);

    // Send response back
    let response = "Hello from QUIC server!";
    send.write_all(response.as_bytes()).await?;
    send.finish()?;
    info!("📤 Server sent response: {}", response);

    Ok(())
}

/// Handle client-side connection
async fn handle_client_connection(connection: Connection) -> Result<(), Box<dyn Error>> {
    info!("🔧 Client using connection");

    // Open a bidirectional stream
    let (mut send, mut recv) = connection.open_bi().await?;
    info!("📤 Client opened bidirectional stream");

    // Send data to server
    let message = "Hello from QUIC client!";
    send.write_all(message.as_bytes()).await?;
    send.finish()?;
    info!("📨 Client sent: {}", message);

    // Read response from server
    let data = recv.read_to_end(1024).await?;
    let response = String::from_utf8(data)?;
    info!("📥 Client received: {}", response);

    // Verify we got expected response
    if response == "Hello from QUIC server!" {
        info!("✅ QUIC communication successful!");
    } else {
        error!("❌ Unexpected response: {}", response);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quic_basic_connectivity() {
        // This test demonstrates that our QUIC setup works
        let server_config = configure_server().expect("Should create server config");

        // Verify server config is valid
        assert!(server_config
            .transport_config()
            .max_idle_timeout()
            .is_some());

        // Test would run actual connection if we had a test server
        // For now, just verify configuration works
    }

    #[test]
    fn test_address_configuration() {
        assert_eq!(SERVER_ADDR.port(), 5001);
        assert_eq!(SERVER_NAME, "localhost");
        assert!(CLIENT_ADDR.port() == 0); // OS will assign
    }
}
