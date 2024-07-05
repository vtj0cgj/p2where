use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::env;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tun::TunBuilder;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::MaybeTlsStream;

#[derive(Serialize, Deserialize)]
pub enum PeerMessage {
    // Make PeerMessage public
    Connect(String), // IP address to connect to
    Data(Vec<u8>),   // Data to route
}

async fn route_traffic(ws_stream: tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>) {
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

    // Split the WebSocket stream into a sender and receiver
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Task to read from TUN device and send data to the server
    let send_task = tokio::spawn(async move {
        loop {
            let mut buf = vec![0; 1504];
            let n = reader.read(&mut buf).await.unwrap();
            buf.truncate(n);
            let msg = PeerMessage::Data(buf);
            let msg = serde_json::to_string(&msg).unwrap();
            ws_sender.send(Message::Text(msg)).await.unwrap();
        }
    });

    // Task to receive data from the server and write it to the TUN device
    let receive_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    writer.write_all(&data).await.unwrap();
                }
                Ok(Message::Text(text)) => {
                    // Handle text messages if needed
                    eprintln!("Received unexpected text message: {}", text);
                }
                Err(e) => {
                    eprintln!("WebSocket error: {}", e);
                    break;
                }
                _ => (),
            }
        }
    });

    // Await both tasks to run concurrently
    tokio::try_join!(send_task, receive_task).unwrap();
}

#[tokio::main]
async fn main() {
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
    let connect_msg = PeerMessage::Connect(peer_addr.clone());
    let connect_msg = serde_json::to_string(&connect_msg).unwrap();
    ws_stream.send(Message::Text(connect_msg)).await.unwrap();

    // Start routing traffic through the server
    route_traffic(ws_stream).await;
}
