# OpenLive-Rust

Rust-first media timing and synchronization core for the OpenLive project. The
repository is being developed independently of the Android application.

## Current implementation

- `rust-sync-core/` — monotonic session clock, JNI exports, unit conversions,
  A/V drift diagnostics, and per-track monotonic presentation-timestamp (PTS)
  normalization.
- `rust-sync-core/src/av_sync.rs` — host-testable A/V timing primitives that
  do not depend on Android APIs.
- `rust-sync-core/src/audio_clock.rs` — maps audio sample-frame indices to
  session-relative PTS using integer arithmetic, avoiding cumulative rounding drift.
- `rust-sync-core/src/bounded_queue.rs` — bounded FIFO with explicit
  `DropNewest` and `DropOldest` overflow policies and drop counters.
- `rust-sync-core/src/shared_queue.rs` — mutex-protected, cloneable queue for
  producer/consumer threads, with consistent capacity/length/drop snapshots.
- `rust-sync-core/src/media_packet.rs` — typed audio/video packets with validated
  session-relative PTS, optional positive duration, sequence number, and payload.
- `rust-sync-core/src/scheduler.rs` — per-track packet PTS ordering diagnostics,
  repeated/backwards timestamp counters, and audio/video high-water-mark skew.
- `rust-sync-core/src/discontinuity.rs` — per-track detection of repeated PTS,
  forward gaps, out-of-order timestamps, and large backward jumps; detection only,
  leaving recovery and timestamp repair to the pipeline owner.
- `rust-sync-core/src/media_scheduler.rs` — bounded per-track packet buffers,
  PTS-sorted insertion, cross-track earliest-PTS selection, and explicit overflow policies.
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

### Bounded media queues

`BoundedQueue` enforces a fixed capacity and makes overload behavior explicit.
Use `DropOldest` for latency-sensitive preview video where fresh frames matter
more than stale ones. Do not silently apply it to encoded audio: audio needs a
separate back-pressure or discontinuity policy. The base primitive is not internally thread-safe; use `SharedBoundedQueue`
for simple concurrent hand-off. The shared wrapper uses a mutex and is not a
blocking channel: consumers must provide their own wake-up/polling strategy.
`MediaPacket` standardizes packet metadata in microseconds; it does not perform
capture, encoding, scheduling, or muxing. `PacketOrderTracker` classifies PTS
progression independently per track and exposes a high-water-mark skew. It does
not reorder or discard packets; the pipeline owner must choose that policy.
`MediaScheduler` is a separate bounded queue primitive: it sorts buffered packets
by PTS and emits the earliest queued packet across tracks. `peek_next_pts_us()` lets
the consumer inspect the next deadline candidate without consuming a packet, and
`is_full(kind)` exposes per-track capacity pressure. Its overflow policy is per
track, and it is not a real-time playback loop; the caller must still manage clock
deadlines, decoder readiness, audio-device back-pressure, and discontinuities.
Keep audio and video PTS in the same session-relative timebase before comparing them.

### Audio sample clock

`AudioSampleClock` maps a sample-frame index to microseconds using the actual
sample rate. Advance it by frames per channel, not the total number of
interleaved channel samples. Deriving PTS from the absolute sample index avoids
cumulative rounding errors from repeatedly adding rounded buffer durations.
The current implementation is a timing primitive; it does not capture audio or
know how many frames a device has actually played.

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
2. Connect the sample clock to real audio capture/playback position and resampling.
3. Add bounded audio/video queues and explicit back-pressure policies.
4. Add a media pipeline abstraction for capture, encoding, muxing, and output.
5. Integrate Android audio/codec backends through a narrow JNI boundary.
6. Add instrumentation and long-duration device tests for drift, pause/restart,
   concurrent stream/record outputs, and failure recovery.


## OpenLive media port (in progress)

The `port/openlive-audio-pipeline` branch starts moving Android-independent media policy from `Roadsearch/OpenLive` into `rust-sync-core`:

- `audio_dsp.rs` ports stereo PCM microphone processing: high-pass filter, noise gate, gain, pan, mute, soft limiter, and dBFS meters.
- `thermal.rs` ports thermal levels, battery/system status mapping, bitrate factors, and one-step cooldown behavior. Android still reads the sensors and schedules the cooldown.
- `stream.rs` ports stream configuration helpers, RTMP URL assembly, thermal bitrate selection, and extra destination filtering.
- `pipeline.rs` models streaming/recording lifecycle decisions and shared clock/encoder shutdown effects. Kotlin/Android still executes those effects.

This is an incremental Rust core port. Camera2, AudioRecord, MediaCodec, EGL/OpenGL, MediaProjection, RootEncoder RTMP, MediaMuxer, Android permissions, and app UI remain in Kotlin. The new media modules passed a local `cargo check`; Android/JNI integration is still pending.
