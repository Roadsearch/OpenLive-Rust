# OpenLive-Rust

Rust session-clock prototype for the Android OpenLive streaming app.

## Scope

The first migration slice is a shared, session-relative monotonic clock. It
prevents starting a second output (for example recording while streaming) from
resetting the shared A/V timestamp origin.

Android camera capture, audio capture/mixing, MediaCodec, OpenGL, RTMP/MP4
outputs, lifecycle, and UI remain in Kotlin while this clock is tested.

## Layout

- `rust-sync-core/` — Rust clock, JNI exports, and unit tests.
- `android-integration/` — Kotlin JNI bridge and output-lifecycle adapter.
- `.github/workflows/rust-ci.yml` — formatting, unit tests, and Rust checks.
- `.github/workflows/android-native.yml` — builds Android shared libraries for
  `arm64-v8a` and `armeabi-v7a`, then publishes them as a workflow artifact.

## Status

The Rust CI and Android native cross-build have passed in GitHub Actions.
The Kotlin adapter is still a reference integration, not a complete Android
Gradle module and not yet wired into `Roadsearch/OpenLive`. No physical-device
JNI or end-to-end A/V synchronization test has been performed yet.

## Local Rust tests

```sh
cd rust-sync-core
cargo fmt --all -- --check
cargo test --all-targets
cargo check --all-targets
```

## Android integration contract

Use one `RustLiveClock` owned by the pipeline. Call
`startOutput("stream")` when live output starts and
`startOutput("recording")` when local recording starts. Stop each output by
its matching ID. The native session starts on the first output and ends only
when the final output stops. Stopping an unknown output ID has no effect.

The Kotlin bridge expects `librust_sync_core.so` to be packaged for each device
ABI under `app/src/main/jniLibs/<abi>/`. The Android Actions workflow publishes
the compiled libraries as an artifact; download and copy them before enabling
the bridge in the app.

**Do not replace production `LiveClock` until the native library loads on a
physical device and end-to-end audio/video synchronization tests pass.**
