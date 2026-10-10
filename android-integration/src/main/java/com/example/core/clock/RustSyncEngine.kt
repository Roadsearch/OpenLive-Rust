package com.example.core.clock

/**
 * JNI bridge to the Rust monotonic clock.
 *
 * Keep one shared timeline for live streaming and recording. Do not create a
 * new clock per output or reset it when starting/stopping a sink.
 */
class RustSyncEngine {
    companion object {
        init {
            System.loadLibrary("rust_sync_core")
        }
    }

    /** Monotonic nanoseconds from the shared Rust epoch. Not wall-clock time. */
    external fun nativeNowNs(): Long

    /** Converts a non-negative nanosecond timestamp to microseconds. */
    external fun nativeNsToUs(timestampNs: Long): Long

    /** Converts a non-negative nanosecond timestamp to milliseconds. */
    external fun nativeNsToMs(timestampNs: Long): Long
}
