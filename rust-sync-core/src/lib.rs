//! Shared session clock primitives for coordinating live output and recording.
//! Starting a second output is idempotent: it never resets the active session.

use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[derive(Default)]
struct SessionClock {
    started_at: Option<Instant>,
}

impl SessionClock {
    fn start(&mut self) {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
    }

    fn stop(&mut self) {
        self.started_at = None;
    }

    fn now_ns(&self) -> u64 {
        self.started_at
            .map(|start| start.elapsed().as_nanos().min(i64::MAX as u128) as u64)
            .unwrap_or(0)
    }
}

static SESSION: OnceLock<Mutex<SessionClock>> = OnceLock::new();

fn session() -> &'static Mutex<SessionClock> {
    SESSION.get_or_init(|| Mutex::new(SessionClock::default()))
}

/// Starts the shared timeline if no output session is active.
/// Calling this while a session is active deliberately does nothing.
pub fn start_session() {
    session().lock().unwrap_or_else(|e| e.into_inner()).start();
}

/// Stops the shared timeline. Call only after all outputs have stopped.
pub fn stop_session() {
    session().lock().unwrap_or_else(|e| e.into_inner()).stop();
}

/// Nanoseconds elapsed since the active session began, or zero when stopped.
pub fn session_now_ns() -> u64 {
    session().lock().unwrap_or_else(|e| e.into_inner()).now_ns()
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
    fn session_clock_starts_stops_and_restarts() {
        let mut clock = SessionClock::default();
        assert_eq!(clock.now_ns(), 0);

        clock.start();
        let first = clock.now_ns();
        clock.start(); // Starting a second output must not reset the origin.
        assert!(clock.now_ns() >= first);

        clock.stop();
        assert_eq!(clock.now_ns(), 0);
        clock.start();
        assert!(clock.now_ns() >= 0);
    }

    #[test]
    fn timestamp_units_convert_downward() {
        assert_eq!(ns_to_us(1_999), 1);
        assert_eq!(ns_to_ms(1_999_999), 1);
        assert_eq!(ns_to_us(0), 0);
        assert_eq!(ns_to_ms(u64::MAX), u64::MAX / 1_000_000);
    }
}
