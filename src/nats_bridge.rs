use async_nats::Client;
use base64::Engine as _;
use futures_util::StreamExt;
use log::{error, info};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc};

#[derive(Deserialize)]
struct TtsChunk {
    data: String, // base64 PCM
    chunk_index: u32,
    is_final: bool,
}

#[derive(Deserialize)]
struct TtsStatus {
    status: String,
    speaker_id: String,
}

pub struct NatsBridge {
    client: Client,
    tts_tx: broadcast::Sender<Vec<u8>>,
    state_tx: broadcast::Sender<String>,
}

impl NatsBridge {
    pub fn new(
        client: Client,
        tts_tx: broadcast::Sender<Vec<u8>>,
        state_tx: broadcast::Sender<String>,
    ) -> Self {
        Self { client, tts_tx, state_tx }
    }

    pub async fn run_subscribers(self) {
        let tts_sub = self.subscribe_tts();
        let state_sub = self.subscribe_states();
        tokio::join!(tts_sub, state_sub);
    }

    async fn subscribe_tts(&self) {
        let mut sub = match self.client.subscribe("tts.audio_chunk.*").await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to subscribe to tts.audio_chunk.*: {}", e);
                return;
            }
        };
        info!("Subscribed to tts.audio_chunk.*");

        while let Some(msg) = sub.next().await {
            let Ok(chunk) = serde_json::from_slice::<TtsChunk>(&msg.payload) else {
                continue;
            };
            let Ok(pcm) = base64::engine::general_purpose::STANDARD.decode(&chunk.data) else {
                continue;
            };
            // Broadcast to WS clients and local playback
            let _ = self.tts_tx.send(pcm);
        }
    }

    async fn subscribe_states(&self) {
        // Subscribe to multiple state-related subjects
        let subjects = [
            "wake_word.detected",
            "tts.status.*",
            "speech.transcribed",
        ];

        let mut handles = Vec::new();
        for subj in subjects {
            let client = self.client.clone();
            let state_tx = self.state_tx.clone();
            handles.push(tokio::spawn(async move {
                let mut sub = match client.subscribe(subj).await {
                    Ok(s) => s,
                    Err(e) => {
                        error!("Failed to subscribe to {}: {}", subj, e);
                        return;
                    }
                };
                info!("Subscribed to {}", subj);

                while let Some(msg) = sub.next().await {
                    let state = match msg.subject.as_str() {
                        s if s == "wake_word.detected" => "listening".to_string(),
                        s if s.starts_with("tts.status.") => {
                            if let Ok(status) = serde_json::from_slice::<TtsStatus>(&msg.payload) {
                                match status.status.as_str() {
                                    "started" => "speaking".to_string(),
                                    "completed" | "interrupted" => "idle".to_string(),
                                    _ => continue,
                                }
                            } else {
                                continue;
                            }
                        }
                        s if s == "speech.transcribed" => "processing".to_string(),
                        _ => continue,
                    };

                    let event = serde_json::json!({
                        "type": "state_changed",
                        "state": state,
                    })
                    .to_string();
                    let _ = state_tx.send(event);
                }
            }));
        }

        for h in handles {
            let _ = h.await;
        }
    }
}

pub async fn publish_audio_chunks(
    client: Client,
    mut rx: mpsc::Receiver<(String, Vec<u8>)>,
) {
    info!("Audio publisher ready");
    while let Some((device_id, pcm_data)) = rx.recv().await {
        let payload = serde_json::json!({
            "data": base64::engine::general_purpose::STANDARD.encode(&pcm_data),
            "device_id": device_id,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            "vad_active": true,
        });
        let subject: String = "mordomo.audio.chunk".to_string();
        let data: bytes::Bytes = serde_json::to_vec(&payload).unwrap_or_default().into();
        if let Err(e) = client
            .publish(subject, data)
            .await
        {
            error!("Failed to publish audio chunk: {}", e);
        }
    }
}
