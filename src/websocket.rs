use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc};
use warp::ws::{Message, WebSocket};
use warp::Filter;

#[derive(Deserialize)]
struct AudioMessage {
    #[serde(rename = "type")]
    msg_type: String,
    device_id: Option<String>,
    #[serde(default)]
    data: Option<Vec<u8>>,
}

pub async fn run_server(
    port: u16,
    tts_tx: broadcast::Sender<Vec<u8>>,
    state_tx: broadcast::Sender<String>,
    audio_in_tx: mpsc::Sender<(String, Vec<u8>)>,
) {
    let tts_tx = warp::any().map(move || tts_tx.clone());
    let state_tx = warp::any().map(move || state_tx.clone());
    let audio_in_tx = warp::any().map(move || audio_in_tx.clone());

    let ws_route = warp::path("audio")
        .and(warp::ws())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(tts_tx)
        .and(state_tx)
        .and(audio_in_tx)
        .map(
            |ws: warp::ws::Ws,
             params: std::collections::HashMap<String, String>,
             tts_tx: broadcast::Sender<Vec<u8>>,
             state_tx: broadcast::Sender<String>,
             audio_in_tx: mpsc::Sender<(String, Vec<u8>)>| {
                let device_id = params
                    .get("device_id")
                    .cloned()
                    .unwrap_or_else(|| "unknown".into());
                ws.on_upgrade(move |socket| {
                    handle_connection(socket, device_id, tts_tx, state_tx, audio_in_tx)
                })
            },
        );

    let health = warp::path("health").map(|| warp::reply::json(&serde_json::json!({"status": "ok"})));

    let routes = ws_route.or(health);

    warp::serve(routes).run(([0, 0, 0, 0], port)).await;
}

async fn handle_connection(
    ws: WebSocket,
    device_id: String,
    tts_tx: broadcast::Sender<Vec<u8>>,
    state_tx: broadcast::Sender<String>,
    audio_in_tx: mpsc::Sender<(String, Vec<u8>)>,
) {
    info!("Device connected: {}", device_id);
    let (mut ws_tx, mut ws_rx) = ws.split();

    let mut tts_rx = tts_tx.subscribe();
    let mut state_rx = state_tx.subscribe();

    // Task: Forward TTS audio + state to WS client
    let device_id_clone = device_id.clone();
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = tts_rx.recv() => {
                    match result {
                        Ok(pcm) => {
                            if ws_tx.send(Message::binary(pcm)).await.is_err() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("WS client {} lagged {} chunks", device_id_clone, n);
                        }
                        Err(_) => break,
                    }
                }
                result = state_rx.recv() => {
                    match result {
                        Ok(state_json) => {
                            if ws_tx.send(Message::text(state_json)).await.is_err() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(_) => break,
                    }
                }
            }
        }
    });

    // Task: Receive audio from WS client → NATS
    let recv_device_id = device_id.clone();
    while let Some(Ok(msg)) = ws_rx.next().await {
        if msg.is_binary() {
            let _ = audio_in_tx
                .send((recv_device_id.clone(), msg.into_bytes()))
                .await;
        } else if msg.is_text() {
            // Handle ping/pong or JSON messages
            if let Ok(text) = msg.to_str() {
                if text.contains("\"ping\"") {
                    // Ignore pings, connection is alive
                }
            }
        } else if msg.is_close() {
            break;
        }
    }

    send_task.abort();
    info!("Device disconnected: {}", device_id);
}
