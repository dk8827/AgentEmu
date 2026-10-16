//! Bounded single-producer/single-consumer PCM transport. Atomic sample slots
//! avoid locks and allocations in the audio callback. A generation invalidates
//! in-flight audio on pause/reset without racing the consumer's read cursor.
use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering},
};
const CAPACITY: usize = 8192;
pub struct AudioQueue {
    samples: Box<[AtomicU64]>,
    read: AtomicUsize,
    write: AtomicUsize,
    generation: AtomicU32,
    volume: AtomicU32,
    prebuffer: AtomicUsize,
    pub total: AtomicU64,
    pub underruns: AtomicUsize,
    pub overruns: AtomicUsize,
}
#[derive(Clone)]
pub struct AudioSink {
    queue: Arc<AudioQueue>,
    generation: u32,
}
pub struct AudioReader {
    queue: Arc<AudioQueue>,
    generation: u32,
    phase: f64,
    warming: bool,
    pair: Option<(f32, f32)>,
}
impl AudioQueue {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            samples: (0..CAPACITY).map(|_| AtomicU64::new(0)).collect(),
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
            generation: AtomicU32::new(1),
            volume: AtomicU32::new(0.6f32.to_bits()),
            prebuffer: AtomicUsize::new(1600),
            total: AtomicU64::new(0),
            underruns: AtomicUsize::new(0),
            overruns: AtomicUsize::new(0),
        })
    }
    pub fn sink(self: &Arc<Self>) -> AudioSink {
        AudioSink {
            queue: self.clone(),
            generation: self.generation.load(Ordering::Acquire),
        }
    }
    pub fn reader(self: &Arc<Self>) -> AudioReader {
        AudioReader {
            queue: self.clone(),
            generation: 0,
            phase: 0.,
            warming: true,
            pair: None,
        }
    }
    pub fn clear(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
    pub fn volume(&self, v: f32) {
        self.volume
            .store(v.clamp(0., 1.).to_bits(), Ordering::Relaxed);
    }
    pub fn queued(&self) -> usize {
        self.write
            .load(Ordering::Acquire)
            .wrapping_sub(self.read.load(Ordering::Acquire))
            .min(CAPACITY)
    }
}
impl AudioSink {
    pub fn push(&self, samples: &[f32]) {
        if self.generation != self.queue.generation.load(Ordering::Acquire) {
            return;
        }
        let mut write = self.queue.write.load(Ordering::Relaxed);
        for &sample in samples {
            if write.wrapping_sub(self.queue.read.load(Ordering::Acquire)) >= CAPACITY {
                self.queue.overruns.fetch_add(1, Ordering::Relaxed);
                break;
            }
            self.queue.samples[write % CAPACITY].store(
                (u64::from(self.generation) << 32) | u64::from(sample.to_bits()),
                Ordering::Relaxed,
            );
            write = write.wrapping_add(1);
            self.queue.write.store(write, Ordering::Release);
            self.queue.total.fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn queued(&self) -> usize {
        self.queue.queued()
    }
}
impl AudioReader {
    fn pop(&self) -> Option<f32> {
        let mut read = self.queue.read.load(Ordering::Relaxed);
        while read != self.queue.write.load(Ordering::Acquire) {
            let value = self.queue.samples[read % CAPACITY].load(Ordering::Relaxed);
            read = read.wrapping_add(1);
            self.queue.read.store(read, Ordering::Release);
            if (value >> 32) as u32 == self.generation {
                return Some(f32::from_bits(value as u32));
            }
        }
        None
    }
    pub fn sample(&mut self, ratio: f64) -> f32 {
        let generation = self.queue.generation.load(Ordering::Acquire);
        if generation != self.generation {
            self.generation = generation;
            self.pair = None;
            self.phase = 0.;
            self.warming = true;
        }
        if self.warming {
            // Discard old-generation samples before estimating new prebuffer.
            while self.queue.queued() > 0 {
                let read = self.queue.read.load(Ordering::Relaxed);
                if (self.queue.samples[read % CAPACITY].load(Ordering::Acquire) >> 32) as u32
                    == generation
                {
                    break;
                }
                self.queue
                    .read
                    .store(read.wrapping_add(1), Ordering::Release);
            }
            if self.queue.queued() < self.queue.prebuffer.load(Ordering::Relaxed) {
                return 0.;
            }
            self.warming = false;
        }
        if self.pair.is_none() {
            self.pair = self.pop().zip(self.pop());
        }
        let Some((mut a, mut b)) = self.pair else {
            self.queue.underruns.fetch_add(1, Ordering::Relaxed);
            return 0.;
        };
        let result = a + (b - a) * self.phase as f32;
        self.phase += ratio;
        while self.phase >= 1. {
            self.phase -= 1.;
            a = b;
            if let Some(next) = self.pop() {
                b = next;
            } else {
                self.pair = None;
                self.phase = 0.;
                self.warming = true;
                return result * f32::from_bits(self.queue.volume.load(Ordering::Relaxed));
            }
        }
        self.pair = Some((a, b));
        result * f32::from_bits(self.queue.volume.load(Ordering::Relaxed))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generation_rejects_old_producer_and_buffer_stays_bounded() {
        let q = AudioQueue::new();
        q.volume(1.);
        q.prebuffer.store(0, Ordering::Relaxed);
        let old = q.sink();
        let mut reader = q.reader();
        old.push(&[0.25; 100]);
        assert_eq!(reader.sample(1.), 0.25);
        q.clear();
        old.push(&[0.5; 100]);
        assert_eq!(reader.sample(1.), 0.);
        let new = q.sink();
        new.push(&[0.75; CAPACITY + 200]);
        assert!(q.queued() <= CAPACITY);
        assert_eq!(reader.sample(1.), 0.75);
        assert_eq!(q.overruns.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn resampling_interpolates_and_queue_crosses_threads() {
        let q = AudioQueue::new();
        q.volume(1.);
        q.prebuffer.store(0, Ordering::Relaxed);
        let sink = q.sink();
        std::thread::spawn(move || sink.push(&[0., 0.5, 1., 0.5, 0.]))
            .join()
            .unwrap();
        let mut r = q.reader();
        assert_eq!(r.sample(0.5), 0.);
        assert_eq!(r.sample(0.5), 0.25);
        assert_eq!(r.sample(0.5), 0.5);
    }
}
