# OpenLive-Rust

Rust-first synchronization core for the Android OpenLive streaming app.

## Scope

This repository starts with a small, testable monotonic clock for audio/video timestamps. Android camera, audio capture, MediaCodec, OpenGL, lifecycle, and UI remain in Kotlin until the Rust core is validated.

## Layout

- `rust-sync-core/` — Rust library and unit tests.
- `app/` — Android integration notes and JNI wrapper example (to be connected to the original Android project).
- `.github/workflows/` — Rust CI.

## Status

Initial scaffold only. JNI/Android integration and end-to-end A/V tests are not yet validated.

## Local tests

```sh
cd rust-sync-core
cargo test
```

## Integration strategy

1. Establish one monotonic timeline shared by live streaming and local recording.
2. Keep timestamps in nanoseconds internally; convert only at API boundaries.
3. Never reset the clock when starting or stopping an individual output.
4. Integrate through JNI only after the Android package and native library loading path are confirmed.
