use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use session_engine::audio_queue::{AudioQueue, AudioSink};
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub struct Audio {
    queue: Arc<AudioQueue>,
    _stream: cpal::Stream,
    failed: Arc<AtomicBool>,
}
impl Audio {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no audio output device")?;
        let supported = device.default_output_config()?;
        let config: cpal::StreamConfig = supported.clone().into();
        let queue = AudioQueue::new();
        let failed = Arc::new(AtomicBool::new(false));
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                build::<f32>(&device, &config, queue.clone(), failed.clone())?
            }
            cpal::SampleFormat::I16 => {
                build::<i16>(&device, &config, queue.clone(), failed.clone())?
            }
            cpal::SampleFormat::U16 => {
                build::<u16>(&device, &config, queue.clone(), failed.clone())?
            }
            _ => return Err("unsupported audio output sample format".into()),
        };
        stream.play()?;
        Ok(Self {
            queue,
            _stream: stream,
            failed,
        })
    }
    pub fn samples(&self) -> u64 {
        self.queue.total.load(Ordering::Relaxed)
    }
    pub fn sink(&self) -> AudioSink {
        self.queue.sink()
    }
    pub fn volume(&self, v: f32) {
        self.queue.volume(v);
    }
    pub fn active(&self) -> bool {
        !self.failed.load(Ordering::Relaxed)
    }
    pub fn queued(&self) -> usize {
        self.queue.queued()
    }
    pub fn clear(&self) {
        self.queue.clear();
    }
}
fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    queue: Arc<AudioQueue>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let channels = config.channels as usize;
    let ratio = 48000. / config.sample_rate.0 as f64;
    let mut reader = queue.reader();
    device.build_output_stream(
        config,
        move |output: &mut [T], _| {
            for frame in output.chunks_mut(channels) {
                let value = reader.sample(ratio);
                for sample in frame {
                    *sample = T::from_sample(value);
                }
            }
        },
        move |error| {
            failed.store(true, Ordering::Relaxed);
            eprintln!("Audio output error: {error}");
        },
        None,
    )
}
