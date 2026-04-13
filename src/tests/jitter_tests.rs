use rand::Rng;
use std::collections::HashSet;

/// Replicate the jitter logic from server.rs to test it in isolation
fn compute_jittered_delay(delay: u64) -> u64 {
    let jitter_range = delay / 4;
    if jitter_range > 0 {
        let offset = rand::rng().random_range(0..=jitter_range * 2);
        delay.saturating_sub(jitter_range) + offset
    } else {
        delay
    }
}

// ─── Jitter range tests ─────────────────────────────────────────────────────

#[test]
fn test_jitter_stays_within_bounds() {
    let delay = 20u64;
    let min = delay - delay / 4; // 15
    let max = delay + delay / 4; // 25

    for _ in 0..1000 {
        let jittered = compute_jittered_delay(delay);
        assert!(
            jittered >= min && jittered <= max,
            "jittered={jittered} out of range [{min}, {max}] for delay={delay}"
        );
    }
}

#[test]
fn test_jitter_produces_varied_values() {
    let delay = 100u64;
    let values: HashSet<u64> = (0..200).map(|_| compute_jittered_delay(delay)).collect();
    // With ±25% of 100, range is 75..=125, so 51 possible values.
    // 200 samples should hit at least 10 distinct values.
    assert!(
        values.len() >= 10,
        "expected at least 10 distinct values, got {}",
        values.len()
    );
}

#[test]
fn test_jitter_zero_delay() {
    // delay=0 → jitter_range=0 → should always return 0
    for _ in 0..100 {
        assert_eq!(compute_jittered_delay(0), 0);
    }
}

#[test]
fn test_jitter_small_delay() {
    // delay=1 → jitter_range=0 → should always return 1
    for _ in 0..100 {
        assert_eq!(compute_jittered_delay(1), 1);
    }
}

#[test]
fn test_jitter_delay_3() {
    // delay=3 → jitter_range=0 → no jitter
    for _ in 0..100 {
        assert_eq!(compute_jittered_delay(3), 3);
    }
}

#[test]
fn test_jitter_delay_4() {
    // delay=4 → jitter_range=1 → range: 3..=5
    for _ in 0..100 {
        let j = compute_jittered_delay(4);
        assert!((3..=5).contains(&j), "got {j} for delay=4");
    }
}

#[test]
fn test_jitter_large_delay() {
    let delay = 3600u64; // 1 hour
    let min = delay - delay / 4; // 2700
    let max = delay + delay / 4; // 4500

    for _ in 0..500 {
        let j = compute_jittered_delay(delay);
        assert!(
            j >= min && j <= max,
            "jittered={j} out of range [{min}, {max}] for delay={delay}"
        );
    }
}
