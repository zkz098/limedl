use super::types::MirrorResource;

/// Configuration options for scoring candidate mirrors.
#[derive(Debug, Clone)]
pub struct MirrorScoringConfig {
    /// Preferred country code (ISO 3166-1 alpha-2, lowercase, e.g. "cn", "us").
    pub preferred_location: Option<String>,
    /// Preferred protocol: "https" or "http".
    pub preferred_protocol: String,
}

impl Default for MirrorScoringConfig {
    fn default() -> Self {
        Self {
            preferred_location: None,
            preferred_protocol: "https".to_string(),
        }
    }
}

/// Calculate a composite score for a mirror candidate. Higher score is better.
pub fn calculate_mirror_score(
    resource: &MirrorResource,
    rtt_ms: Option<u64>,
    consecutive_errors: u32,
    config: &MirrorScoringConfig,
) -> f64 {
    let mut score = 0.0;

    // 1. Priority score: server-assigned priority (1 is highest)
    let priority_val = resource.priority.max(1) as f64;
    score += 100.0 / priority_val;

    // 2. Protocol preference
    let is_https = resource.url.starts_with("https://");
    if is_https && config.preferred_protocol == "https" {
        score += 15.0;
    }

    // 3. Geographic proximity score
    if let (Some(preferred), Some(loc)) = (&config.preferred_location, &resource.location) {
        if loc.eq_ignore_ascii_case(preferred) {
            score += 50.0;
        } else if are_regions_close(preferred, loc) {
            score += 25.0;
        }
    }

    // 4. Latency score (from real RTT / TTFB probe)
    if let Some(rtt) = rtt_ms {
        let latency_score = match rtt {
            0..=50 => 80.0,
            51..=100 => 60.0,
            101..=200 => 40.0,
            201..=400 => 20.0,
            401..=800 => 5.0,
            _ => 0.0,
        };
        score += latency_score;
    }

    // 5. Error penalty: strongly penalize failing mirrors
    score -= (consecutive_errors as f64) * 40.0;

    score
}

fn are_regions_close(loc_a: &str, loc_b: &str) -> bool {
    let a = loc_a.to_ascii_lowercase();
    let b = loc_b.to_ascii_lowercase();

    // East Asia cluster
    let east_asia = ["cn", "hk", "mo", "tw", "jp", "kr", "sg"];
    if east_asia.contains(&a.as_str()) && east_asia.contains(&b.as_str()) {
        return true;
    }

    // Europe cluster
    let europe = ["de", "fr", "nl", "gb", "uk", "ch", "at", "se", "no", "fi", "pl", "it", "es"];
    if europe.contains(&a.as_str()) && europe.contains(&b.as_str()) {
        return true;
    }

    // North America cluster
    let na = ["us", "ca", "mx"];
    if na.contains(&a.as_str()) && na.contains(&b.as_str()) {
        return true;
    }

    false
}
