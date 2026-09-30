use std::time::{Duration, Instant};

pub use super::types::AdaptiveProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

#[derive(Debug, Default)]
pub struct AimdState {
    pub last_sample_bytes: u64,
    pub last_sample_at: Option<Instant>,
    pub last_throughput: Option<f64>,
    pub cooldown_until: Option<Instant>,
    pub consecutive_good_samples: u32,
    pub consecutive_bad_samples: u32,
    pub recent_penalty: bool,
    pub throughput_sample_count: u32,
    pub throughput_sum: f64,
    pub peak_throughput: f64,
    pub penalty_count: u32,
    pub last_direction: Option<Direction>,
    pub oscillation_count: u32,
    pub hysteresis_lock_until: Option<Instant>,
}

impl AimdState {
    pub fn initial(_profile: Option<AdaptiveProfile>, _desired: Option<usize>) -> Self {
        Self::default()
    }

    pub fn sample_throughput(&mut self, downloaded_bytes: u64, now: Instant) -> Option<f64> {
        let throughput = match self.last_sample_at {
            Some(last_at) => {
                let elapsed = now.duration_since(last_at).as_secs_f64();
                if elapsed > 0.0 {
                    Some(downloaded_bytes.saturating_sub(self.last_sample_bytes) as f64 / elapsed)
                } else {
                    None
                }
            }
            None => None,
        };

        self.last_sample_bytes = downloaded_bytes;
        self.last_sample_at = Some(now);
        throughput
    }

    pub fn record_sample(&mut self, throughput: f64) {
        if throughput <= 0.0 || !throughput.is_finite() {
            return;
        }

        self.throughput_sample_count = self.throughput_sample_count.saturating_add(1);
        self.throughput_sum += throughput;
        self.peak_throughput = self.peak_throughput.max(throughput);
    }
}

pub fn initial_desired_threads(profile: AdaptiveProfile, cap: usize) -> usize {
    let cap = cap.max(1);
    match profile {
        AdaptiveProfile::Conservative => (cap as f64 * 0.5).ceil() as usize,
        AdaptiveProfile::Balanced => (cap as f64 * 0.75).ceil() as usize,
        AdaptiveProfile::Aggressive => cap,
    }
}

pub fn reduce_threads(current: usize, profile: AdaptiveProfile, min_threads: usize) -> usize {
    let reduced = match profile {
        AdaptiveProfile::Conservative => ((current as f64) * 0.7).ceil() as usize,
        AdaptiveProfile::Balanced | AdaptiveProfile::Aggressive => {
            ((current as f64) * 0.5).ceil() as usize
        }
    };
    reduced.max(min_threads.max(1))
}

pub fn cooldown_for_profile(profile: AdaptiveProfile) -> Duration {
    match profile {
        AdaptiveProfile::Conservative => Duration::from_secs(4),
        AdaptiveProfile::Balanced => Duration::from_secs(3),
        AdaptiveProfile::Aggressive => Duration::from_secs(2),
    }
}

#[cfg(test)]
mod tests;
