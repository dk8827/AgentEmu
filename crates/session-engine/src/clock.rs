//! Clock accounting belongs to execution, independent of presentation cadence.
#[derive(Default)]
pub struct FrameClock {
    last: Option<f64>,
    budget: f64,
    pub dropped: f64,
}
impl FrameClock {
    pub fn reset(&mut self) {
        self.last = None;
        self.budget = 0.;
    }
    pub fn frames(&mut self, now: f64, rate: f64, running: bool) -> usize {
        self.frames_with_audio(now, rate, running, None)
    }
    pub fn frames_with_audio(
        &mut self,
        now: f64,
        rate: f64,
        running: bool,
        queued: Option<usize>,
    ) -> usize {
        if !running {
            self.reset();
            return 0;
        }
        if !now.is_finite() || !rate.is_finite() || rate <= 0. {
            return 0;
        }
        let elapsed = self
            .last
            .replace(now)
            .map_or(1. / rate, |last| (now - last).max(0.));
        // Slowly follow device-clock drift; never pitch-shift by a large amount
        // to compensate for UI stalls. Silent playback uses wall time exactly.
        let correction = queued.map_or(0., |n| {
            ((1600. - n as f64) / 48000. * 0.05).clamp(-0.005, 0.005)
        });
        let budget = self.budget + elapsed * rate * (1. + correction);
        self.dropped += (budget - 6.).max(0.);
        self.budget = budget.min(6.);
        let count = (self.budget.floor() as usize).min(4);
        self.budget -= count as f64;
        count
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slow_presentations_count_time_and_late_resume_is_bounded() {
        let mut c = FrameClock::default();
        c.frames(0., 60., true);
        let count: usize = (1..=500)
            .map(|i| c.frames(i as f64 * 0.02, 60., true))
            .sum();
        assert!((599..=601).contains(&count));
        assert_eq!(c.frames(100., 60., true), 4);
        assert!(c.dropped > 100.);
        assert_eq!(c.frames(101., 60., false), 0);
        assert_eq!(c.frames(200., 60., true), 1);
    }
}
