mod client;
mod server;
use futures_util::SinkExt;
use std::env;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite;

async fn client_run() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <server_addr> <peer_addr>", args[0]);
        return;
    }
    let server_addr = &args[1];
    let peer_addr = &args[2];

    let (mut ws_stream, _) = connect_async(format!("ws://{}", server_addr))
        .await
        .expect("Failed to connect to server");

    // Send connect message to the server
    let connect_msg = crate::client::PeerMessage::Connect(peer_addr.clone());
    let connect_msg = serde_json::to_string(&connect_msg).unwrap();
    ws_stream
        .send(tungstenite::protocol::Message(connect_msg))
        .await
        .unwrap();

    // Start routing traffic through the server
    route_traffic(ws_stream).await;
}

fn main() {}
