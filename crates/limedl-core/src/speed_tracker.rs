//! Real-time sliding-window transfer speed calculation for download tasks.
//!
//! Tracks downloaded bytes in small circular time buckets to compute accurate,
//! smooth, and responsive transfer rates without heap allocation during hot paths.

use std::time::{Duration, Instant};

/// Sliding-window speed tracker for calculating real-time transfer rates.
///
/// Uses a circular buffer of time buckets (10 buckets of 200ms = 2.0s total window)
/// to provide smooth, responsive, and accurate speed measurements without dynamic allocations.
#[derive(Debug, Clone)]
pub struct SpeedTracker {
    buckets: [(Instant, u64); Self::BUCKET_COUNT],
    current_index: usize,
    first_sample_at: Option<Instant>,
    last_sample_at: Option<Instant>,
    initialized: bool,
}

impl SpeedTracker {
    pub const BUCKET_COUNT: usize = 10;
    pub const BUCKET_DURATION: Duration = Duration::from_millis(200);
    pub const TOTAL_WINDOW: Duration = Duration::from_millis(2000);

    pub fn new() -> Self {
        Self::default()
    }

    /// Reset tracker state (e.g. on pause, cancel, or restart).
    pub fn reset(&mut self) {
        self.initialized = false;
        self.current_index = 0;
        self.first_sample_at = None;
        self.last_sample_at = None;
        let now = Instant::now();
        self.buckets.fill((now, 0));
    }

    /// Returns true if at least one transfer sample was recorded in this session.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Record transferred bytes at a given instant.
    pub fn record_bytes(&mut self, bytes: u64, now: Instant) {
        if bytes == 0 {
            return;
        }

        // If uninitialized or more than TOTAL_WINDOW has elapsed since last transfer (stall/gap),
        // restart fresh from `now`.
        if !self.initialized
            || self
                .last_sample_at
                .is_some_and(|last| now.saturating_duration_since(last) >= Self::TOTAL_WINDOW)
        {
            self.buckets.fill((now, 0));
            self.buckets[0] = (now, bytes);
            self.current_index = 0;
            self.first_sample_at = Some(now);
            self.last_sample_at = Some(now);
            self.initialized = true;
            return;
        }

        self.last_sample_at = Some(now);

        let (current_time, current_bytes) = &mut self.buckets[self.current_index];
        if now < *current_time {
            // Clock anomaly / non-monotonic timestamp: just accumulate
            *current_bytes = current_bytes.saturating_add(bytes);
            return;
        }

        let elapsed = now.saturating_duration_since(*current_time);
        if elapsed < Self::BUCKET_DURATION {
            *current_bytes = current_bytes.saturating_add(bytes);
        } else {
            let steps = (elapsed.as_millis() / Self::BUCKET_DURATION.as_millis()) as usize;
            if steps >= Self::BUCKET_COUNT {
                self.buckets.fill((now, 0));
                self.current_index = 0;
                self.buckets[0] = (now, bytes);
                self.first_sample_at = Some(now);
            } else {
                for i in 1..=steps {
                    let idx = (self.current_index + i) % Self::BUCKET_COUNT;
                    self.buckets[idx] = (now, 0);
                }
                self.current_index = (self.current_index + steps) % Self::BUCKET_COUNT;
                self.buckets[self.current_index] = (now, bytes);
            }
        }
    }

    /// Calculate the current transfer speed in bytes per second.
    /// Returns None if uninitialized or if no bytes were recorded within the sliding window.
    pub fn current_speed(&self, now: Instant) -> Option<f64> {
        if !self.initialized {
            return None;
        }

        let cutoff = now.checked_sub(Self::TOTAL_WINDOW)?;
        let mut total_bytes: u64 = 0;

        for &(bucket_time, bytes) in &self.buckets {
            if bucket_time >= cutoff && bucket_time <= now && bytes > 0 {
                total_bytes = total_bytes.saturating_add(bytes);
            }
        }

        if total_bytes == 0 {
            return None;
        }

        let elapsed = match self.first_sample_at {
            Some(first) => now.saturating_duration_since(first),
            None => Self::TOTAL_WINDOW,
        };

        let effective_elapsed = elapsed.clamp(Self::BUCKET_DURATION, Self::TOTAL_WINDOW).as_secs_f64();
        if effective_elapsed <= 0.0 {
            return None;
        }

        let speed = total_bytes as f64 / effective_elapsed;
        if speed > 0.0 && speed.is_finite() {
            Some(speed)
        } else {
            None
        }
    }
}

impl Default for SpeedTracker {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            buckets: [(now, 0); Self::BUCKET_COUNT],
            current_index: 0,
            first_sample_at: None,
            last_sample_at: None,
            initialized: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uninitialized_returns_none() {
        let tracker = SpeedTracker::new();
        assert!(!tracker.is_initialized());
        assert_eq!(tracker.current_speed(Instant::now()), None);
    }

    #[test]
    fn initial_ramp_up_speed_calculation() {
        let mut tracker = SpeedTracker::new();
        let t0 = Instant::now();

        // 1 MB transferred at t0
        tracker.record_bytes(1_000_000, t0);
        assert!(tracker.is_initialized());

        // At t0 + 100ms (clamped to BUCKET_DURATION = 200ms)
        let speed = tracker.current_speed(t0 + Duration::from_millis(100)).unwrap();
        // 1_000_000 / 0.2 = 5_000_000
        assert!((speed - 5_000_000.0).abs() < 1.0, "got {speed}");

        // At t0 + 500ms, transfer another 1 MB
        tracker.record_bytes(1_000_000, t0 + Duration::from_millis(500));
        let speed = tracker.current_speed(t0 + Duration::from_millis(500)).unwrap();
        // 2_000_000 / 0.5 = 4_000_000
        assert!((speed - 4_000_000.0).abs() < 1.0, "got {speed}");
    }

    #[test]
    fn steady_state_speed_calculation() {
        let mut tracker = SpeedTracker::new();
        let t0 = Instant::now();

        // Feed 1 MB every 200ms for 2.0s (10 buckets = 10 MB total)
        for i in 0..10 {
            tracker.record_bytes(1_000_000, t0 + Duration::from_millis(i * 200));
        }

        // At t0 + 2000ms: 10 MB over 2.0s = 5 MB/s
        let now = t0 + Duration::from_millis(2000);
        let speed = tracker.current_speed(now).unwrap();
        assert!((speed - 5_000_000.0).abs() < 100.0, "got {speed}");

        // Continue feeding at same rate for another 2 seconds
        for i in 10..20 {
            tracker.record_bytes(1_000_000, t0 + Duration::from_millis(i * 200));
        }
        let now = t0 + Duration::from_millis(4000);
        let speed = tracker.current_speed(now).unwrap();
        assert!((speed - 5_000_000.0).abs() < 100.0, "got {speed}");
    }

    #[test]
    fn stall_decays_to_none_after_window_expires() {
        let mut tracker = SpeedTracker::new();
        let t0 = Instant::now();

        tracker.record_bytes(5_000_000, t0);
        assert!(tracker.current_speed(t0 + Duration::from_millis(500)).is_some());

        // After window expires (> 2000ms), speed should be None
        let stalled_at = t0 + Duration::from_millis(2500);
        assert_eq!(tracker.current_speed(stalled_at), None);
    }

    #[test]
    fn resume_after_stall_starts_fresh() {
        let mut tracker = SpeedTracker::new();
        let t0 = Instant::now();

        tracker.record_bytes(5_000_000, t0);

        // Gap of 10 seconds
        let t1 = t0 + Duration::from_secs(10);
        tracker.record_bytes(2_000_000, t1);

        // Should only reflect the new 2MB transfer
        let speed = tracker.current_speed(t1 + Duration::from_millis(200)).unwrap();
        // 2_000_000 / 0.2 = 10_000_000
        assert!((speed - 10_000_000.0).abs() < 1.0, "got {speed}");
    }

    #[test]
    fn reset_clears_all_history() {
        let mut tracker = SpeedTracker::new();
        let t0 = Instant::now();

        tracker.record_bytes(5_000_000, t0);
        assert!(tracker.is_initialized());

        tracker.reset();
        assert!(!tracker.is_initialized());
        assert_eq!(tracker.current_speed(t0 + Duration::from_millis(100)), None);
    }
}
