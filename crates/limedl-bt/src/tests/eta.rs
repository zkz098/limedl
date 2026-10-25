//! ETA estimation.

use super::*;

#[test]
fn test_estimate_eta_normal() {
    assert_eq!(estimate_eta(1000, 500, Some(100.0)), Some(5));
}

#[test]
fn test_estimate_eta_zero_speed() {
    assert_eq!(estimate_eta(1000, 500, Some(0.0)), None);
}

#[test]
fn test_estimate_eta_completed() {
    assert_eq!(estimate_eta(1000, 1000, Some(100.0)), None);
}

#[test]
fn test_estimate_eta_over_downloaded() {
    assert_eq!(estimate_eta(1000, 1500, Some(100.0)), None);
}

#[test]
fn test_estimate_eta_none_speed() {
    assert_eq!(estimate_eta(1000, 500, None), None);
}

#[test]
fn test_estimate_eta_small_speed() {
    // 1 byte remaining at 0.5 B/s => ceil(1.0 / 0.5) = 2
    assert_eq!(estimate_eta(1000, 999, Some(0.5)), Some(2));
}

#[test]
fn test_estimate_eta_exact_division() {
    // 100 bytes remaining at 50 B/s => 2 seconds
    assert_eq!(estimate_eta(200, 100, Some(50.0)), Some(2));
}
