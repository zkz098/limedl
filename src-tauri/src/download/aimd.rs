use std::time::{Duration, Instant};

use super::types::AdaptiveProfile;

#[derive(Debug, Default)]
pub(crate) struct AimdState {
    pub(crate) last_sample_bytes: u64,
    pub(crate) last_sample_at: Option<Instant>,
    pub(crate) last_throughput: Option<f64>,
    pub(crate) cooldown_until: Option<Instant>,
    pub(crate) consecutive_good_samples: u32,
    pub(crate) consecutive_bad_samples: u32,
    pub(crate) recent_penalty: bool,
    pub(crate) throughput_sample_count: u32,
    pub(crate) throughput_sum: f64,
    pub(crate) peak_throughput: f64,
    pub(crate) penalty_count: u32,
}

impl AimdState {
    pub(crate) fn initial(
        _profile: Option<AdaptiveProfile>,
        _desired: Option<usize>,
    ) -> Self {
        Self::default()
    }

    pub(crate) fn sample_throughput(
        &mut self,
        downloaded_bytes: u64,
        now: Instant,
    ) -> Option<f64> {
        let throughput = match self.last_sample_at {
            Some(last_at) => {
                let elapsed = now.duration_since(last_at).as_secs_f64();
                if elapsed > 0.0 {
                    Some(
                        downloaded_bytes
                            .saturating_sub(self.last_sample_bytes) as f64
                            / elapsed,
                    )
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

    pub(crate) fn record_sample(&mut self, throughput: f64) {
        if throughput <= 0.0 || !throughput.is_finite() {
            return;
        }

        self.throughput_sample_count = self.throughput_sample_count.saturating_add(1);
        self.throughput_sum += throughput;
        self.peak_throughput = self.peak_throughput.max(throughput);
    }
}

pub(crate) fn initial_desired_threads(profile: AdaptiveProfile) -> usize {
    match profile {
        AdaptiveProfile::Conservative => 1,
        AdaptiveProfile::Balanced => 2,
        AdaptiveProfile::Aggressive => 4,
    }
}

pub(crate) fn reduce_threads(
    current: usize,
    profile: AdaptiveProfile,
    min_threads: usize,
) -> usize {
    let reduced = match profile {
        AdaptiveProfile::Conservative => ((current as f64) * 0.7).ceil() as usize,
        AdaptiveProfile::Balanced | AdaptiveProfile::Aggressive => {
            ((current as f64) * 0.5).ceil() as usize
        }
    };
    reduced.max(min_threads.max(1))
}

pub(crate) fn cooldown_for_profile(profile: AdaptiveProfile) -> Duration {
    match profile {
        AdaptiveProfile::Conservative => Duration::from_secs(8),
        AdaptiveProfile::Balanced => Duration::from_secs(6),
        AdaptiveProfile::Aggressive => Duration::from_secs(4),
    }
}
