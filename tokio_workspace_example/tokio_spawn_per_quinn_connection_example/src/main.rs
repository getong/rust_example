use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

use quinn::{
    Certificate, CertificateChain, ClientConfig, Endpoint, EndpointBuilder, PrivateKey, ServerConfig,
    ServerConfigBuilder,
};

async fn handle_connection(conn: quinn::Connection) -> Result<(), Box<dyn Error>> {
    let mut stream = conn.open_uni().await?;

    loop {
        let mut buf = [0; 1024];
        let len = match stream.read(&mut buf).await {
            Ok(len) => len,
            Err(_) => {
                break;
            }
        };

        if stream.write_all(&buf[..len]).await.is_err() {
            break;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cert = Certificate::from_pem(include_bytes!("cert.pem"))?;
    let key = PrivateKey::from_pem(include_bytes!("key.pem"))?;
    let chain = CertificateChain::from_certs(vec![cert.clone()]);

    let mut server_config = ServerConfigBuilder::default();
    server_config.certificate(chain, key)?;
    let server_config = Arc::new(server_config.build());

    let mut endpoint_builder = Endpoint::builder();
    endpoint_builder.listen(server_config.clone());
    let (endpoint, _) = endpoint_builder.bind(&"[::]:0".parse().unwrap())?;

    let mut incoming = endpoint.incoming();

    println!("Listening on {}", endpoint.local_addr()?);

    while let Some(conn) = incoming.next().await {
        let conn = conn?;
        let server_config = server_config.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(conn).await {
                eprintln!("connection error: {}", e);
            }
        });
    }

    Ok(())
}