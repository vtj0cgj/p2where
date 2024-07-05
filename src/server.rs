use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tun::TunBuilder;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Serialize, Deserialize)]
enum PeerMessage {
    Connect(String), // IP address to connect to
    Data(Vec<u8>),   // Data to route
}

type Peers = Arc<Mutex<HashMap<String, tokio_tungstenite::WebSocketStream<TcpStream>>>>;

async fn handle_connection(raw_stream: TcpStream, peers: Peers) {
    let ws_stream = match accept_async(raw_stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("Error during websocket handshake: {}", e);
            return;
        }
    };
    let (_ws_sender, mut ws_receiver) = ws_stream.split();

    while let Some(msg) = ws_receiver.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Failed to read message: {}", e);
                continue;
            }
        };
        if msg.is_text() {
            let peer_msg: PeerMessage = match serde_json::from_str(msg.to_text().unwrap()) {
                Ok(pm) => pm,
                Err(e) => {
                    eprintln!("Invalid message format: {}", e);
                    continue;
                }
            };
            match peer_msg {
                PeerMessage::Connect(peer_addr) => {
                    let new_stream = match TcpStream::connect(peer_addr.clone()).await {
                        Ok(ns) => ns,
                        Err(e) => {
                            eprintln!("Failed to connect to peer: {}", e);
                            continue;
                        }
                    };
                    let peer_ws_stream = match accept_async(new_stream).await {
                        Ok(pws) => pws,
                        Err(e) => {
                            eprintln!("WebSocket handshake failed: {}", e);
                            continue;
                        }
                    };
                    peers.lock().await.insert(peer_addr, peer_ws_stream);
                }
                PeerMessage::Data(data) => {
                    let mut peers_guard = peers.lock().await;
                    for (_addr, peer) in peers_guard.iter_mut() {
                        if let Err(e) = peer.send(Message::Binary(data.clone())).await {
                            eprintln!("Failed to send data to peer: {}", e);
                        }
                    }
                }
            }
        }
    }
}

async fn route_traffic(peers: Peers) {
    // Create a TUN device
    let tun = TunBuilder::new()
        .name("tun0") // if name is empty, then it is set by kernel.
        .tap(false) // false for TUN device, true for TAP device
        .packet_info(false) // false to read/write IP, true to read/write ethernet
        .up()
        .try_build()
        .unwrap();

    // Split the TUN interface into a reader and writer
    let (mut reader, mut writer) = tokio::io::split(tun);

    loop {
        let mut buf = vec![0; 1504];
        let n = reader.read(&mut buf).await.unwrap();
        buf.truncate(n);

        // Send the data to all peers
        let mut peers_guard = peers.lock().await;
        for (_addr, peer) in peers_guard.iter_mut() {
            if let Err(e) = peer.send(Message::Binary(buf.clone())).await {
                eprintln!("Failed to send data to peer: {}", e);
            }
        }

        // If you want to receive data from peers and write it back to the TUN interface
        for (_addr, peer) in peers_guard.iter_mut() {
            if let Some(Ok(Message::Binary(data))) = peer.next().await {
                writer.write_all(&data).await.unwrap();
            }
        }
    }
}

#[tokio::main]
async fn run_server() {
    let addr = "0.0.0.0:8080";
    let listener = TcpListener::bind(addr).await.expect("Failed to bind");
    let peers: Peers = Arc::new(Mutex::new(HashMap::new()));

    println!("Listening on: {}", addr);

    // Spawn the traffic routing task
    let peers_clone = peers.clone();
    tokio::spawn(async move {
        route_traffic(peers_clone).await;
    });

    while let Ok((stream, _)) = listener.accept().await {
        let peers = peers.clone();
        tokio::spawn(async move {
            handle_connection(stream, peers).await;
        });
    }
}
