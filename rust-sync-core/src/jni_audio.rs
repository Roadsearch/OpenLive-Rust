//! JNI adapter for the PCM microphone processor.
//!
//! The Kotlin owner must stop/join its audio thread before `close()` so the
//! native handle cannot be destroyed while a processing call is in flight.

use std::sync::Mutex;

use jni::objects::{JClass, JShortArray};
use jni::sys::{jboolean, jfloat, jfloatArray, jint, jlong};
use jni::JNIEnv;

use crate::audio_dsp::{AudioConfig, ChannelSettings, MicProcessor};

type ProcessorHandle = Mutex<MicProcessor>;

#[no_mangle]
pub extern "system" fn Java_com_example_core_audio_RustAudioProcessor_nativeCreate(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    Box::into_raw(Box::new(ProcessorHandle::new(MicProcessor::default()))) as jlong
}

#[no_mangle]
pub extern "system" fn Java_com_example_core_audio_RustAudioProcessor_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        // Safety: the matching Kotlin owner calls this once and only after its
        // synchronized audio calls have finished.
        unsafe {
            drop(Box::from_raw(handle as *mut ProcessorHandle));
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_example_core_audio_RustAudioProcessor_nativeProcess(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    pcm: JShortArray,
    sample_rate: jint,
    volume: jfloat,
    gain_db: jfloat,
    pan: jfloat,
    muted: jboolean,
    high_pass_enabled: jboolean,
    noise_gate_threshold: jfloat,
) -> jfloatArray {
    if handle == 0 || sample_rate <= 0 {
        return std::ptr::null_mut();
    }
    let Ok(length) = env.get_array_length(&pcm) else {
        return std::ptr::null_mut();
    };
    let mut samples = vec![0_i16; length as usize];
    if env.get_short_array_region(&pcm, 0, &mut samples).is_err() {
        return std::ptr::null_mut();
    }

    let processor = unsafe { &*(handle as *const ProcessorHandle) };
    let mut processor = processor
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let config = AudioConfig {
        sample_rate: sample_rate as u32,
        mic: ChannelSettings {
            volume,
            gain_db,
            pan,
            muted: muted != 0,
        },
        master_volume: 1.0,
        high_pass_enabled: high_pass_enabled != 0,
        noise_gate_threshold,
    };
    let Ok(meter) = processor.process_stereo(&mut samples, config) else {
        return std::ptr::null_mut();
    };

    if env.set_short_array_region(&pcm, 0, &samples).is_err() {
        return std::ptr::null_mut();
    }
    let Ok(result) = env.new_float_array(2) else {
        return std::ptr::null_mut();
    };
    if env
        .set_float_array_region(&result, 0, &[meter.peak_dbfs, meter.rms_dbfs])
        .is_err()
    {
        return std::ptr::null_mut();
    }
    result.into_raw()
}
