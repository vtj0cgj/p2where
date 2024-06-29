mod dht;

use dht::{Peer, DHT};
use rustls::server::NoClientAuth;
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{
    rustls::{Certificate, PrivateKey, ServerConfig},
    TlsAcceptor,
};

#[tokio::main]
async fn main() {
    let dht = Arc::new(Mutex::new(DHT::new()));

    let dht_clone = dht.clone();
    tokio::spawn(async move {
        let peer = dht_clone.lock().unwrap().add_peer("127.0.0.1", 8080);
        println!("Added peer: {:?}", peer);

        let peers = dht_clone.lock().unwrap().find_peers();
        println!("Peers in DHT: {:?}", peers);
    });

    start_server(dht.clone()).await;
}

async fn start_server(dht: Arc<Mutex<DHT>>) {
    let cert_file = &mut BufReader::new(File::open("cert.pem").unwrap());
    let key_file = &mut BufReader::new(File::open("key.pem").unwrap());

    let certs: Vec<Certificate> = certs(cert_file)
        .unwrap()
        .into_iter()
        .map(Certificate)
        .collect();
    let mut keys: Vec<PrivateKey> = pkcs8_private_keys(key_file)
        .unwrap()
        .into_iter()
        .map(PrivateKey)
        .collect();

    let mut config = ServerConfig::new(NoClientAuth::new());
    config.set_single_cert(certs, keys.remove(0)).unwrap();

    let acceptor = TlsAcceptor::from(Arc::new(config));

    let listener = TcpListener::bind("0.0.0.0:8443").await.unwrap();
    println!("Server listening on port 8443");

    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        let acceptor = acceptor.clone();
        tokio::spawn(async move {
            handle_connection(acceptor, stream, addr).await;
        });
    }
}

async fn handle_connection(acceptor: TlsAcceptor, stream: TcpStream, addr: std::net::SocketAddr) {
    println!("Accepted connection from {:?}", addr);

    match acceptor.accept(stream).await {
        Ok(mut tls_stream) => {
            println!("TLS handshake completed with {:?}", addr);

            let (mut reader, mut writer) = io::split(tls_stream);

            // Example of reading data from the client
            let mut buf = vec![0; 1024];
            match reader.read(&mut buf).await {
                Ok(n) if n == 0 => {
                    // Connection was closed
                    println!("Connection closed by {:?}", addr);
                }
                Ok(n) => {
                    // Process the data
                    println!("Received data from {:?}: {:?}", addr, &buf[..n]);
                }
                Err(e) => {
                    println!("Failed to read from connection: {:?}", e);
                }
            }

            // Example of sending data to the client
            let response = b"Hello from server!";
            if let Err(e) = writer.write_all(response).await {
                println!("Failed to write to connection: {:?}", e);
            }
        }
        Err(e) => {
            println!("TLS handshake failed with {:?}: {:?}", addr, e);
        }
    }
}
