mod config;
mod nats_bridge;
mod playback;
mod websocket;

use log::info;
use std::sync::Arc;
use tokio::sync::broadcast;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cfg = config::Config::from_env();
    info!("Audio Bridge starting on port {}", cfg.ws_port);

    // Channel: NATS TTS chunks → WebSocket clients + local playback
    let (tts_tx, _) = broadcast::channel::<Vec<u8>>(256);

    // Channel: NATS state changes → WebSocket clients
    let (state_tx, _) = broadcast::channel::<String>(64);

    // Channel: WebSocket audio in → NATS publish
    let (audio_in_tx, audio_in_rx) = tokio::sync::mpsc::channel::<(String, Vec<u8>)>(256);

    let nats_client = async_nats::connect(&cfg.nats_url).await?;
    info!("Connected to NATS at {}", cfg.nats_url);

    // Spawn NATS subscriber (TTS chunks + state events → broadcast)
    let nats_sub = nats_bridge::NatsBridge::new(
        nats_client.clone(),
        tts_tx.clone(),
        state_tx.clone(),
    );
    tokio::spawn(nats_sub.run_subscribers());

    // Spawn NATS publisher (WebSocket audio in → NATS)
    tokio::spawn(nats_bridge::publish_audio_chunks(nats_client.clone(), audio_in_rx));

    // Spawn local audio playback (TTS → speaker)
    if cfg.enable_local_playback {
        let playback_rx = tts_tx.subscribe();
        tokio::spawn(async move {
            if let Err(e) = playback::run_playback(playback_rx).await {
                log::error!("Local playback error: {}", e);
            }
        });
        info!("Local audio playback enabled");
    }

    // Start WebSocket server
    info!("WebSocket server listening on 0.0.0.0:{}", cfg.ws_port);
    websocket::run_server(cfg.ws_port, tts_tx, state_tx, audio_in_tx).await;

    Ok(())
}
