//! Small monotonic clock primitives for coordinating live output and recording.
//! The clock deliberately has no reset-on-output-start API: starting or stopping
//! one sink must not change timestamps observed by another sink.

use std::sync::OnceLock;
use std::time::Instant;

static EPOCH: OnceLock<Instant> = OnceLock::new();

/// Nanoseconds elapsed since this process first initialized the shared epoch.
pub fn monotonic_now_ns() -> u64 {
    let epoch = EPOCH.get_or_init(Instant::now);
    epoch.elapsed().as_nanos().min(u64::MAX as u128) as u64
}

/// Converts a monotonic nanosecond timestamp to microseconds, rounding down.
pub fn ns_to_us(timestamp_ns: u64) -> u64 {
    timestamp_ns / 1_000
}

/// Converts a monotonic nanosecond timestamp to milliseconds, rounding down.
pub fn ns_to_ms(timestamp_ns: u64) -> u64 {
    timestamp_ns / 1_000_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_is_monotonic_non_decreasing() {
        let a = monotonic_now_ns();
        let b = monotonic_now_ns();
        assert!(b >= a);
    }

    #[test]
    fn timestamp_units_convert_downward() {
        assert_eq!(ns_to_us(1_999), 1);
        assert_eq!(ns_to_ms(1_999_999), 1);
    }
}
