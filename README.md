# OpenLive-Rust

Rust-first media timing and synchronization core for the OpenLive project. The
repository is being developed independently of the Android application.

## Current implementation

- `rust-sync-core/` — monotonic session clock, JNI exports, unit conversions,
  A/V drift diagnostics, and per-track monotonic presentation-timestamp (PTS)
  normalization.
- `rust-sync-core/src/av_sync.rs` — host-testable A/V timing primitives that
  do not depend on Android APIs.
- `android-integration/` — Kotlin JNI bridge and output-lifecycle adapter,
  with JVM tests. This is a reference integration, not a complete Android app.
- `.github/workflows/rust-ci.yml` — formatting, Rust unit tests, and checks.
- `.github/workflows/android-native.yml` — builds Android shared libraries for
  `arm64-v8a` and `armeabi-v7a`.
- `.github/workflows/kotlin-ci.yml` — tests the Kotlin adapter on the JVM.

## Synchronization contracts

### Shared session clock

The clock is monotonic and session-relative; it is not UTC. Starting a second
output must not reset the origin. The session should stop only after every
output has stopped. Keep a single owner for each pipeline and validate timestamp
units at the MediaCodec/audio-backend boundaries.

### A/V drift monitor

`AvSyncMonitor` accepts audio and video PTS already converted to the same
microsecond timebase. It reports signed drift as **video PTS minus audio PTS**:
positive means video timestamps are ahead; negative means they are behind.
It is diagnostic only; it does not yet schedule, drop, or delay media.

### Monotonic PTS normalization

Create one `MonotonicPts` per output track. It clamps negative values and
repairs repeated or backward timestamps. This is a safety utility for invalid
inputs, not a substitute for preserving valid source PTS, DTS/PTS reordering,
or a proper resampling / audio-device clock model.

## Local checks

```sh
cd rust-sync-core
cargo fmt --all -- --check
cargo test --all-targets
cargo check --all-targets
```

## Android integration status

The Kotlin adapter is not wired into `Roadsearch/OpenLive`. Android CI builds
native libraries and verifies JNI symbols, but no physical-device JNI test or
end-to-end audio/video synchronization test has been performed. Do not treat
the current diagnostics as a complete production streaming engine.

## Roadmap

1. Harden timebase and session/output lifecycle contracts.
2. Introduce deterministic audio timing and sample-index mapping.
3. Add bounded audio/video queues and explicit back-pressure policies.
4. Add a media pipeline abstraction for capture, encoding, muxing, and output.
5. Integrate Android audio/codec backends through a narrow JNI boundary.
6. Add instrumentation and long-duration device tests for drift, pause/restart,
   concurrent stream/record outputs, and failure recovery.
