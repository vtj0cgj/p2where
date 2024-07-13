wuse futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{accept_async, connect_async, tungstenite::protocol::Message};

async fn handle_connection(raw_stream: TcpStream) {
    println!("New connection established!");
    let ws_stream = accept_async(raw_stream)
        .await
        .expect("Error during WebSocket handshake");
    let (mut write, mut read) = ws_stream.split();

    while let Some(msg) = read.next().await {
        let msg = msg.expect("Error reading message");
        if msg.is_binary() {
            println!("Server received binary data");
            write.send(msg).await.expect("Error sending message");
        }
    }
}

async fn start_server(addr: &str) {
    let listener = TcpListener::bind(addr).await.expect("Failed to bind");
    println!("Server running on {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_connection(stream));
    }
}

async fn start_client(addr: &str, forward_addr: &str) {
    let url = url::Url::parse(&format!("ws://{}", addr)).unwrap();
    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    let (mut write, mut read) = ws_stream.split();

    println!("Client connected to {}", addr);

    let forward_stream = TcpStream::connect(forward_addr)
        .await
        .expect("Failed to connect to forward address");
    let forward_stream = Arc::new(Mutex::new(forward_stream));
    let forward_read = forward_stream.clone();
    let forward_write = forward_stream.clone();

    let ws_write = tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            let msg = msg.expect("Error reading message");
            if msg.is_binary() {
                println!("Client received binary data");
                let mut forward_write = forward_write.lock().await;
                forward_write
                    .write_all(&msg.into_data())
                    .await
                    .expect("Error writing to forward stream");
            }
        }
    });

    let ws_read = tokio::spawn(async move {
        let mut buffer = vec![0; 1024];
        let mut forward_read = forward_read.lock().await;
        loop {
            let n = forward_read
                .read(&mut buffer)
                .await
                .expect("Error reading from forward stream");
            if n == 0 {
                break;
            }
            println!("Client forwarding binary data");
            write
                .send(Message::Binary(buffer[..n].to_vec()))
                .await
                .expect("Error sending message");
        }
    });

    ws_write.await.expect("WebSocket write task failed");
    ws_read.await.expect("WebSocket read task failed");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <server|client> <address> [forward_address]",
            args[0]
        );
        return;
    }

    let mode = &args[1];
    let addr = &args[2];

    match mode.as_str() {
        "server" => start_server(addr).await,
        "client" => {
            if args.len() < 4 {
                eprintln!("Client mode requires a forward address");
                return;
            }
            let forward_addr = &args[3];
            start_client(addr, forward_addr).await;
        }
        _ => eprintln!("Unknown mode: {}", mode),
    }
}
