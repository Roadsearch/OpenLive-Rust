# Android JNI integration

This folder contains an example Kotlin bridge for the Rust session clock. It is
not a complete Android Gradle module and is not yet wired into the original
OpenLive app.

## Contract

- Kotlin package/class: `com.example.core.clock.RustSyncEngine`
- Native library name: `rust_sync_core` (Android file: `librust_sync_core.so`)
- `nativeNowNs()` returns nanoseconds relative to the active session start.
- `nativeStartSession()` is idempotent: starting a second output does not reset
  the timeline.
- `nativeStopSession()` clears the session clock. Call it only after both live
  streaming and local recording have stopped.
- The clock is monotonic during a session and is not UTC/wall-clock time.

## Integration checklist

1. Copy the Kotlin file into the Android app source set.
2. Build/package the Rust shared library for each target ABI and place it under
   `app/src/main/jniLibs/<abi>/`.
3. Keep a single clock instance owned by `LivePipeline`.
4. Call start only when the first output starts; call stop only when no output
   remains active. If recording starts while streaming is active (or vice
   versa), do not reset the clock.
5. Check MediaCodec and muxer timestamp units before adapting the app timebase.
6. Do not replace production timing until end-to-end A/V sync tests pass.

## Important

The JNI bridge is a scaffold. It has not yet been integrated into
`Roadsearch/OpenLive`, and the Rust session clock must not replace
`LiveClock` until app lifecycle handling and timestamp behavior have been
verified on a physical Android device.
