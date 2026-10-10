# Android JNI integration

This folder contains an example Kotlin bridge for the Rust clock. It is not a
complete Android Gradle module and is not yet wired into the original OpenLive
app.

## Contract

- Kotlin package/class: `com.example.core.clock.RustSyncEngine`
- Native library name: `rust_sync_core` (Android file: `librust_sync_core.so`)
- Timestamp unit returned by `nativeNowNs()`: nanoseconds
- The epoch is monotonic and process-local, not UTC/wall-clock time.
- Use one shared timeline for streaming and recording; never reset it when an
  output starts or stops.

## Integration checklist

1. Copy the Kotlin file into the Android app source set.
2. Build/package the Rust shared library for each target ABI and place it under
   `app/src/main/jniLibs/<abi>/`.
3. Confirm exported JNI names with `nm -D` and run on a physical device.
4. Check MediaCodec and muxer timestamp units before adapting the app timebase.
5. Do not replace production timing until end-to-end A/V sync tests pass.
