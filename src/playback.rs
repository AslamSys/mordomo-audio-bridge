use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{error, info, warn};
use tokio::sync::broadcast;

pub async fn run_playback(
    mut rx: broadcast::Receiver<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("No output audio device found")?;

    info!("Using output device: {}", device.name().unwrap_or_default());

    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(22050),
        buffer_size: cpal::BufferSize::Default,
    };

    let (sample_tx, sample_rx) = std::sync::mpsc::channel::<Vec<u8>>();

    let stream = device.build_output_stream(
        &config,
        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
            let mut offset = 0;
            // Try to fill from buffered PCM
            while offset < data.len() {
                match sample_rx.try_recv() {
                    Ok(pcm) => {
                        let samples: &[i16] = unsafe {
                            std::slice::from_raw_parts(
                                pcm.as_ptr() as *const i16,
                                pcm.len() / 2,
                            )
                        };
                        let to_copy = samples.len().min(data.len() - offset);
                        data[offset..offset + to_copy].copy_from_slice(&samples[..to_copy]);
                        offset += to_copy;
                    }
                    Err(_) => {
                        // Fill rest with silence
                        for sample in &mut data[offset..] {
                            *sample = 0;
                        }
                        break;
                    }
                }
            }
        },
        |err| error!("Playback stream error: {}", err),
        None,
    )?;

    stream.play()?;
    info!("Local playback stream started");

    loop {
        match rx.recv().await {
            Ok(pcm) => {
                let _ = sample_tx.send(pcm);
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("Playback lagged, skipped {} chunks", n);
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }

    Ok(())
}
