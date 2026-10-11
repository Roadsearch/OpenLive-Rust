//! PCM processing ported from `AudioMixer.kt`.
//!
//! Buffers are interleaved signed 16-bit PCM. `process_stereo` preserves the
//! source behavior: mic preprocessing, per-channel gain/pan/mute, master gain,
//! then the same soft limiter and peak/RMS dBFS measurements.

use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelSettings {
    pub volume: f32,
    pub gain_db: f32,
    /// -1.0 is left, 0.0 is center, 1.0 is right.
    pub pan: f32,
    pub muted: bool,
}

impl Default for ChannelSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            gain_db: 0.0,
            pan: 0.0,
            muted: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meter {
    pub peak_dbfs: f32,
    pub rms_dbfs: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub mic: ChannelSettings,
    pub master_volume: f32,
    pub high_pass_enabled: bool,
    /// Same normalized threshold and gating behavior as OpenLive's mixer.
    pub noise_gate_threshold: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioError {
    ZeroSampleRate,
    OddStereoSampleCount,
}

#[derive(Debug, Clone)]
pub struct MicProcessor {
    previous_input: [f32; 2],
    previous_output: [f32; 2],
    gate_gain: f32,
    gate_hold_blocks: u8,
}

impl Default for MicProcessor {
    fn default() -> Self {
        Self {
            previous_input: [0.0; 2],
            previous_output: [0.0; 2],
            gate_gain: 1.0,
            gate_hold_blocks: 0,
        }
    }
}

impl MicProcessor {
    /// Process one interleaved stereo PCM block in place.
    pub fn process_stereo(
        &mut self,
        pcm: &mut [i16],
        config: AudioConfig,
    ) -> Result<Meter, AudioError> {
        if config.sample_rate == 0 {
            return Err(AudioError::ZeroSampleRate);
        }
        if pcm.len() % 2 != 0 {
            return Err(AudioError::OddStereoSampleCount);
        }

        if config.high_pass_enabled {
            let a = 1.0 / (1.0 + 2.0 * PI * 80.0 / config.sample_rate as f32);
            for (index, sample) in pcm.iter_mut().enumerate() {
                let channel = index % 2;
                let x = *sample as f32;
                let y = a * (self.previous_output[channel] + x - self.previous_input[channel]);
                self.previous_input[channel] = x;
                self.previous_output[channel] = y;
                *sample = y.clamp(-32768.0, 32767.0) as i16;
            }
        }

        let gate = config.noise_gate_threshold;
        if gate > 0.01 && !pcm.is_empty() {
            let rms = (pcm
                .iter()
                .map(|&sample| (sample as f64).powi(2))
                .sum::<f64>()
                / pcm.len() as f64)
                .sqrt()
                / 32768.0;
            let threshold = (gate * gate * 0.05) as f64;
            if rms >= threshold {
                self.gate_hold_blocks = 8;
            } else {
                self.gate_hold_blocks = self.gate_hold_blocks.saturating_sub(1);
            }
            let target = if self.gate_hold_blocks > 0 { 1.0 } else { 0.0 };
            let start_gain = self.gate_gain;
            self.gate_gain += (target - self.gate_gain) * 0.4;
            if self.gate_gain < 0.999 || start_gain < 0.999 {
                let denom = pcm.len() as f32;
                for (index, sample) in pcm.iter_mut().enumerate() {
                    let gain = start_gain + (self.gate_gain - start_gain) * (index as f32 / denom);
                    *sample = (*sample as f32 * gain) as i16;
                }
            }
        } else {
            self.gate_gain = 1.0;
        }

        let mic_gain = db_to_linear(config.mic.gain_db)
            * config.mic.volume.clamp(0.0, 1.0)
            * config.master_volume.clamp(0.0, 1.0);
        let pan = config.mic.pan.clamp(-1.0, 1.0);
        let left_gain = if pan > 0.0 { 1.0 - pan } else { 1.0 };
        let right_gain = if pan < 0.0 { 1.0 + pan } else { 1.0 };

        if config.mic.muted {
            pcm.fill(0);
        } else {
            for frame in pcm.chunks_exact_mut(2) {
                frame[0] = soft_limit((frame[0] as f32 * mic_gain * left_gain) as i32) as i16;
                frame[1] = soft_limit((frame[1] as f32 * mic_gain * right_gain) as i32) as i16;
            }
        }

        Ok(Meter {
            peak_dbfs: peak_dbfs(pcm),
            rms_dbfs: rms_dbfs(pcm),
        })
    }
}

pub fn soft_limit(sample: i32) -> i32 {
    const THRESHOLD: i32 = 28_000;
    const MAX: i32 = 32_767;
    // FFI callers may supply extreme gain values; keep the limiter arithmetic
    // defined even when the scaled sample saturated during float-to-int conversion.
    let sample = sample.clamp(-i32::MAX, i32::MAX);
    if sample > THRESHOLD {
        let excess = sample - THRESHOLD;
        (THRESHOLD + (excess as f32 / (1.0 + excess as f32 / (MAX - THRESHOLD) as f32)) as i32)
            .min(MAX)
    } else if sample < -THRESHOLD {
        let excess = -sample - THRESHOLD;
        -(THRESHOLD + (excess as f32 / (1.0 + excess as f32 / (MAX - THRESHOLD) as f32)) as i32)
            .min(MAX)
    } else {
        sample
    }
}

pub fn peak_dbfs(pcm: &[i16]) -> f32 {
    let peak = pcm
        .iter()
        .map(|sample| (*sample as i32).abs())
        .max()
        .unwrap_or(0);
    dbfs(peak as f64)
}

pub fn rms_dbfs(pcm: &[i16]) -> f32 {
    if pcm.is_empty() {
        return -60.0;
    }
    let sum = pcm
        .iter()
        .map(|&sample| (sample as f64).powi(2))
        .sum::<f64>();
    dbfs((sum / pcm.len() as f64).sqrt())
}

fn dbfs(amplitude: f64) -> f32 {
    if amplitude <= 1.0 {
        -60.0
    } else {
        (20.0 * (amplitude / 32767.0).log10()).clamp(-60.0, 0.0) as f32
    }
}

fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
