//! Sample-index based audio clock.
//!
//! Audio timestamps should be derived from the number of frames actually
//! consumed/produced, not by repeatedly adding a rounded duration per buffer.

/// Errors returned when constructing an audio sample clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioClockError {
    ZeroSampleRate,
    NegativeOrigin,
}

/// Maps audio sample-frame indices to session-relative microsecond PTS.
///
/// A sample frame contains one sample per channel (for example, one stereo
/// frame still advances the index by one, not two).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSampleClock {
    sample_rate_hz: u32,
    origin_pts_us: i64,
    next_sample_frame: u64,
}

impl AudioSampleClock {
    pub fn new(sample_rate_hz: u32, origin_pts_us: i64) -> Result<Self, AudioClockError> {
        if sample_rate_hz == 0 {
            return Err(AudioClockError::ZeroSampleRate);
        }
        if origin_pts_us < 0 {
            return Err(AudioClockError::NegativeOrigin);
        }
        Ok(Self {
            sample_rate_hz,
            origin_pts_us,
            next_sample_frame: 0,
        })
    }

    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    /// PTS for a zero-based sample-frame index, relative to the origin.
    /// Uses wide integer math to avoid cumulative rounding error and overflow.
    pub fn pts_us_for_sample(&self, sample_frame: u64) -> i64 {
        let elapsed_us = (sample_frame as u128)
            .saturating_mul(1_000_000)
            / self.sample_rate_hz as u128;
        let total = (self.origin_pts_us as u128).saturating_add(elapsed_us);
        total.min(i64::MAX as u128) as i64
    }

    /// Timestamp of the next sample frame that should be written.
    pub fn next_pts_us(&self) -> i64 {
        self.pts_us_for_sample(self.next_sample_frame)
    }

    pub fn next_sample_frame(&self) -> u64 {
        self.next_sample_frame
    }

    /// Advances by sample frames, not interleaved channel samples.
    pub fn advance(&mut self, sample_frames: u64) {
        self.next_sample_frame = self.next_sample_frame.saturating_add(sample_frames);
    }

    /// Starts a new segment at the supplied session-relative origin.
    pub fn reset(&mut self, origin_pts_us: i64) -> Result<(), AudioClockError> {
        if origin_pts_us < 0 {
            return Err(AudioClockError::NegativeOrigin);
        }
        self.origin_pts_us = origin_pts_us;
        self.next_sample_frame = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_clock_parameters() {
        assert_eq!(
            AudioSampleClock::new(0, 0).unwrap_err(),
            AudioClockError::ZeroSampleRate
        );
        assert_eq!(
            AudioSampleClock::new(48_000, -1).unwrap_err(),
            AudioClockError::NegativeOrigin
        );
    }

    #[test]
    fn maps_sample_indices_without_cumulative_rounding_error() {
        let mut clock = AudioSampleClock::new(48_000, 5_000).unwrap();
        assert_eq!(clock.pts_us_for_sample(0), 5_000);
        assert_eq!(clock.pts_us_for_sample(48_000), 1_005_000);
        assert_eq!(clock.pts_us_for_sample(96_000), 2_005_000);

        clock.advance(480);
        assert_eq!(clock.next_sample_frame(), 480);
        assert_eq!(clock.next_pts_us(), 15_000);
        clock.advance(480);
        assert_eq!(clock.next_pts_us(), 25_000);
    }

    #[test]
    fn counts_sample_frames_not_channels() {
        let mut clock = AudioSampleClock::new(44_100, 0).unwrap();
        // 441 stereo frames are 10 ms, not 20 ms.
        clock.advance(441);
        assert_eq!(clock.next_pts_us(), 10_000);
    }

    #[test]
    fn reset_changes_origin_and_clears_sample_position() {
        let mut clock = AudioSampleClock::new(48_000, 0).unwrap();
        clock.advance(4_800);
        assert_eq!(clock.next_pts_us(), 100_000);
        clock.reset(250_000).unwrap();
        assert_eq!(clock.next_sample_frame(), 0);
        assert_eq!(clock.next_pts_us(), 250_000);
    }

    #[test]
    fn clamps_large_timestamps_and_saturates_sample_index() {
        let mut clock = AudioSampleClock::new(1, i64::MAX).unwrap();
        assert_eq!(clock.pts_us_for_sample(u64::MAX), i64::MAX);
        clock.advance(u64::MAX);
        clock.advance(1);
        assert_eq!(clock.next_sample_frame(), u64::MAX);
        assert_eq!(clock.next_pts_us(), i64::MAX);
    }
}
