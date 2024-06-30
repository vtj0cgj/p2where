use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Serialize, Deserialize)]
enum PeerMessage {
    Connect(String), // IP address to connect to
    Data(Vec<u8>),   // Data to route
}

type Peers = Arc<Mutex<HashMap<String, tokio_tungstenite::WebSocketStream<TcpStream>>>>;

async fn handle_connection(raw_stream: TcpStream, peers: Peers) {
    let ws_stream = accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    while let Some(msg) = ws_receiver.next().await {
        let msg = msg.expect("Failed to read message");
        if msg.is_text() {
            let peer_msg: PeerMessage =
                serde_json::from_str(msg.to_text().unwrap()).expect("Invalid message format");
            match peer_msg {
                PeerMessage::Connect(peer_addr) => {
                    let new_stream = TcpStream::connect(peer_addr)
                        .await
                        .expect("Failed to connect to peer");
                    let peer_ws_stream = accept_async(new_stream)
                        .await
                        .expect("WebSocket handshake failed");
                    peers.lock().unwrap().insert(peer_addr, peer_ws_stream);
                }
                PeerMessage::Data(data) => {
                    for (_addr, peer) in peers.lock().unwrap().iter_mut() {
                        peer.send(Message::Binary(data.clone()))
                            .await
                            .expect("Failed to send data to peer");
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await.expect("Failed to bind");
    let peers: Peers = Arc::new(Mutex::new(HashMap::new()));

    println!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let peers = peers.clone();
        tokio::spawn(async move {
            handle_connection(stream, peers).await;
        });
    }
}
