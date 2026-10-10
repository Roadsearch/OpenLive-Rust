package com.example.core.clock

/**
 * JNI bridge to the Rust session clock.
 *
 * Use one shared instance for the entire LivePipeline. Call startSession()
 * when the first output (live OR recording) starts, and stopSession() only
 * after every output has stopped. Starting a second output is idempotent and
 * does not reset timestamps.
 */
class RustSyncEngine {
    companion object {
        init {
            System.loadLibrary("rust_sync_core")
        }
    }

    /** Starts the session timeline if it is not already active. */
    external fun nativeStartSession()

    /** Stops the timeline; call only when streaming AND recording are stopped. */
    external fun nativeStopSession()

    /** Nanoseconds since the active session started, or zero when stopped. */
    external fun nativeNowNs(): Long

    /** Converts a non-negative nanosecond timestamp to microseconds. */
    external fun nativeNsToUs(timestampNs: Long): Long

    /** Converts a non-negative nanosecond timestamp to milliseconds. */
    external fun nativeNsToMs(timestampNs: Long): Long
}
