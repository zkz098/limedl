use std::time::{Duration, Instant};

pub use super::types::AdaptiveProfile;

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

pub fn initial_desired_threads(profile: AdaptiveProfile) -> usize {
    match profile {
        AdaptiveProfile::Conservative => 1,
        AdaptiveProfile::Balanced => 2,
        AdaptiveProfile::Aggressive => 4,
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
        AdaptiveProfile::Conservative => Duration::from_secs(8),
        AdaptiveProfile::Balanced => Duration::from_secs(6),
        AdaptiveProfile::Aggressive => Duration::from_secs(4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    // ── initial_desired_threads ──────────────────────────────

    #[test]
    fn initial_desired_threads_conservative() {
        assert_eq!(initial_desired_threads(AdaptiveProfile::Conservative), 1);
    }

    #[test]
    fn initial_desired_threads_balanced() {
        assert_eq!(initial_desired_threads(AdaptiveProfile::Balanced), 2);
    }

    #[test]
    fn initial_desired_threads_aggressive() {
        assert_eq!(initial_desired_threads(AdaptiveProfile::Aggressive), 4);
    }

    // ── reduce_threads ──────────────────────────────────────

    #[test]
    fn reduce_threads_conservative_scales_by_0_7() {
        // 10 * 0.7 = 7.0 → ceil → 7
        assert_eq!(reduce_threads(10, AdaptiveProfile::Conservative, 1), 7);
        // 3 * 0.7 = 2.1 → ceil → 3
        assert_eq!(reduce_threads(3, AdaptiveProfile::Conservative, 1), 3);
        // 1 * 0.7 = 0.7 → ceil → 1
        assert_eq!(reduce_threads(1, AdaptiveProfile::Conservative, 1), 1);
    }

    #[test]
    fn reduce_threads_balanced_scales_by_0_5() {
        // 10 * 0.5 = 5.0 → ceil → 5
        assert_eq!(reduce_threads(10, AdaptiveProfile::Balanced, 1), 5);
        // 3 * 0.5 = 1.5 → ceil → 2
        assert_eq!(reduce_threads(3, AdaptiveProfile::Balanced, 1), 2);
    }

    #[test]
    fn reduce_threads_aggressive_scales_by_0_5() {
        assert_eq!(reduce_threads(10, AdaptiveProfile::Aggressive, 1), 5);
        assert_eq!(reduce_threads(3, AdaptiveProfile::Aggressive, 1), 2);
    }

    #[test]
    fn reduce_threads_respects_minimum() {
        // min_threads=3, current=2: 2*0.7=1.4→ceil=2, max(2, max(3,1)=3) = 3
        assert_eq!(reduce_threads(2, AdaptiveProfile::Conservative, 3), 3);
    }

    #[test]
    fn reduce_threads_never_below_one() {
        // min_threads=0, current=1: 1*0.5=0.5→ceil=1, max(1, max(0,1)=1) = 1
        assert_eq!(reduce_threads(1, AdaptiveProfile::Aggressive, 0), 1);
    }

    // ── cooldown_for_profile ───────────────────────────────

    #[test]
    fn cooldown_duration_by_profile() {
        assert_eq!(
            cooldown_for_profile(AdaptiveProfile::Conservative),
            Duration::from_secs(8)
        );
        assert_eq!(
            cooldown_for_profile(AdaptiveProfile::Balanced),
            Duration::from_secs(6)
        );
        assert_eq!(
            cooldown_for_profile(AdaptiveProfile::Aggressive),
            Duration::from_secs(4)
        );
    }

    // ── AimdState ───────────────────────────────────────────

    #[test]
    fn aimd_state_initial_returns_defaults() {
        let state = AimdState::initial(None, None);
        assert_eq!(state.last_sample_bytes, 0);
        assert!(state.last_sample_at.is_none());
        assert!(state.last_throughput.is_none());
        assert!(state.cooldown_until.is_none());
        assert_eq!(state.consecutive_good_samples, 0);
        assert_eq!(state.consecutive_bad_samples, 0);
        assert!(!state.recent_penalty);
        assert_eq!(state.throughput_sample_count, 0);
        assert_eq!(state.throughput_sum, 0.0);
        assert_eq!(state.peak_throughput, 0.0);
        assert_eq!(state.penalty_count, 0);
    }

    #[test]
    fn sample_throughput_first_call_returns_none() {
        let mut state = AimdState::initial(None, None);
        let now = Instant::now();
        let result = state.sample_throughput(1000, now);
        assert!(result.is_none());
        assert_eq!(state.last_sample_bytes, 1000);
        assert_eq!(state.last_sample_at, Some(now));
    }

    #[test]
    fn sample_throughput_second_call_computes_rate() {
        let mut state = AimdState::initial(None, None);
        let t0 = Instant::now();
        state.sample_throughput(0, t0);
        // 1 second later, 1000 bytes transferred → 1000 B/s
        let t1 = t0 + Duration::from_secs(1);
        let rate = state.sample_throughput(1000, t1);
        assert!(rate.is_some());
        assert!((rate.unwrap() - 1000.0).abs() < 1.0);
    }

    #[test]
    fn sample_throughput_zero_elapsed_returns_none() {
        let mut state = AimdState::initial(None, None);
        let now = Instant::now();
        state.sample_throughput(0, now);
        // Same Instant → elapsed = 0
        let result = state.sample_throughput(100, now);
        assert!(result.is_none());
    }

    #[test]
    fn sample_throughput_saturating_sub_handles_wraparound() {
        let mut state = AimdState::initial(None, None);
        let now = Instant::now();
        // First sample at 100 bytes
        state.sample_throughput(100, now);
        // Second sample at 50 bytes (counter reset scenario)
        let later = now + Duration::from_secs(1);
        let rate = state.sample_throughput(50, later);
        assert!(rate.is_some());
        // saturating_sub: 50 - 100 = 0, so rate should be 0
        assert!((rate.unwrap() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn record_sample_positive_updates_stats() {
        let mut state = AimdState::initial(None, None);
        state.record_sample(500.0);
        assert_eq!(state.throughput_sample_count, 1);
        assert!((state.throughput_sum - 500.0).abs() < f64::EPSILON);
        assert!((state.peak_throughput - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn record_sample_tracks_peak_throughput() {
        let mut state = AimdState::initial(None, None);
        state.record_sample(100.0);
        state.record_sample(500.0);
        state.record_sample(200.0);
        assert_eq!(state.throughput_sample_count, 3);
        assert!((state.throughput_sum - 800.0).abs() < f64::EPSILON);
        assert!((state.peak_throughput - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn record_sample_zero_is_ignored() {
        let mut state = AimdState::initial(None, None);
        state.record_sample(0.0);
        assert_eq!(state.throughput_sample_count, 0);
        assert_eq!(state.throughput_sum, 0.0);
        assert_eq!(state.peak_throughput, 0.0);
    }

    #[test]
    fn record_sample_negative_is_ignored() {
        let mut state = AimdState::initial(None, None);
        state.record_sample(-100.0);
        assert_eq!(state.throughput_sample_count, 0);
    }

    #[test]
    fn record_sample_nan_is_ignored() {
        let mut state = AimdState::initial(None, None);
        state.record_sample(f64::NAN);
        assert_eq!(state.throughput_sample_count, 0);
    }

    #[test]
    fn record_sample_infinity_is_ignored() {
        let mut state = AimdState::initial(None, None);
        state.record_sample(f64::INFINITY);
        assert_eq!(state.throughput_sample_count, 0);
    }

    #[test]
    fn record_sample_saturating_add_does_not_overflow() {
        let mut state = AimdState::initial(None, None);
        // Set counter near max
        state.throughput_sample_count = u32::MAX;
        state.record_sample(1.0);
        // Should saturate at u32::MAX
        assert_eq!(state.throughput_sample_count, u32::MAX);
    }
}
