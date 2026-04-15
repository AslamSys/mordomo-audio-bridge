use std::env;

pub struct Config {
    pub nats_url: String,
    pub ws_port: u16,
    pub enable_local_playback: bool,
    pub sample_rate: u32,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            nats_url: env::var("NATS_URL").unwrap_or_else(|_| "nats://nats:4222".into()),
            ws_port: env::var("WS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3100),
            enable_local_playback: env::var("ENABLE_LOCAL_PLAYBACK")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
            sample_rate: env::var("SAMPLE_RATE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(22050),
        }
    }
}
