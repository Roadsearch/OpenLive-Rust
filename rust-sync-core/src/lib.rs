//! Shared session clock primitives for coordinating live output and recording.
//! Starting a second output is idempotent: it never resets the active session.

use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static SESSION_START: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

fn session_state() -> &'static Mutex<Option<Instant>> {
    SESSION_START.get_or_init(|| Mutex::new(None))
}

/// Starts the shared timeline if no output session is active.
/// Calling this while a session is active deliberately does nothing.
pub fn start_session() {
    let mut state = session_state().lock().unwrap_or_else(|e| e.into_inner());
    if state.is_none() {
        *state = Some(Instant::now());
    }
}

/// Stops the shared timeline. Call only after BOTH live streaming and recording stop.
pub fn stop_session() {
    let mut state = session_state().lock().unwrap_or_else(|e| e.into_inner());
    *state = None;
}

/// Nanoseconds elapsed since the active session began, or zero when stopped.
pub fn session_now_ns() -> u64 {
    let state = session_state().lock().unwrap_or_else(|e| e.into_inner());
    match *state {
        Some(start) => start.elapsed().as_nanos().min(i64::MAX as u128) as u64,
        None => 0,
    }
}

/// Converts a non-negative nanosecond timestamp to microseconds, rounding down.
pub fn ns_to_us(timestamp_ns: u64) -> u64 {
    timestamp_ns / 1_000
}

/// Converts a non-negative nanosecond timestamp to milliseconds, rounding down.
pub fn ns_to_ms(timestamp_ns: u64) -> u64 {
    timestamp_ns / 1_000_000
}

// JNI entry points for com.example.core.clock.RustSyncEngine.
#[no_mangle]
pub extern "system" fn Java_com_example_core_clock_RustSyncEngine_nativeStartSession(
    _env: *mut c_void,
    _this: *mut c_void,
) {
    start_session();
}

#[no_mangle]
pub extern "system" fn Java_com_example_core_clock_RustSyncEngine_nativeStopSession(
    _env: *mut c_void,
    _this: *mut c_void,
) {
    stop_session();
}

#[no_mangle]
pub extern "system" fn Java_com_example_core_clock_RustSyncEngine_nativeNowNs(
    _env: *mut c_void,
    _this: *mut c_void,
) -> i64 {
    session_now_ns().min(i64::MAX as u64) as i64
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
    fn stopped_clock_returns_zero() {
        stop_session();
        assert_eq!(session_now_ns(), 0);
    }

    #[test]
    fn timestamp_units_convert_downward() {
        assert_eq!(ns_to_us(1_999), 1);
        assert_eq!(ns_to_ms(1_999_999), 1);
    }
}
