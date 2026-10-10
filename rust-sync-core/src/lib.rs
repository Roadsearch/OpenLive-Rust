//! Small monotonic clock primitives for coordinating live output and recording.
//! The clock deliberately has no reset-on-output-start API: starting or stopping
//! one sink must not change timestamps observed by another sink.

use std::ffi::c_void;
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

// JNI entry points for com.example.core.clock.RustSyncEngine.
#[no_mangle]
pub extern "system" fn Java_com_example_core_clock_RustSyncEngine_nativeNowNs(
    _env: *mut c_void,
    _this: *mut c_void,
) -> i64 {
    monotonic_now_ns().min(i64::MAX as u64) as i64
}

#[no_mangle]
pub extern "system" fn Java_com_example_core_clock_RustSyncEngine_nativeNsToUs(
    _env: *mut c_void,
    _this: *mut c_void,
    timestamp_ns: i64,
) -> i64 {
    if timestamp_ns < 0 {
        return 0;
    }
    ns_to_us(timestamp_ns as u64).min(i64::MAX as u64) as i64
}

#[no_mangle]
pub extern "system" fn Java_com_example_core_clock_RustSyncEngine_nativeNsToMs(
    _env: *mut c_void,
    _this: *mut c_void,
    timestamp_ns: i64,
) -> i64 {
    if timestamp_ns < 0 {
        return 0;
    }
    ns_to_ms(timestamp_ns as u64).min(i64::MAX as u64) as i64
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

    #[test]
    fn repeated_reads_share_the_same_timeline() {
        let first = monotonic_now_ns();
        let second = monotonic_now_ns();
        assert!(second >= first);
    }
}
