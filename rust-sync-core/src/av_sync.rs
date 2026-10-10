//! Audio/video timestamp diagnostics and monotonic PTS normalization.
//!
//! This module is deliberately independent of Android/JNI so timing policy can
//! be tested on every host before it is connected to MediaCodec or an audio sink.

/// Signed drift is video PTS minus audio PTS, in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvSyncSnapshot {
    pub last_video_pts_us: Option<i64>,
    pub last_audio_pts_us: Option<i64>,
    pub video_frames: u64,
    pub audio_samples: u64,
}

impl AvSyncSnapshot {
    /// Returns signed video-minus-audio drift in microseconds, if both clocks
    /// have supplied at least one timestamp.
    pub fn drift_us(self) -> Option<i64> {
        let video = self.last_video_pts_us?;
        let audio = self.last_audio_pts_us?;
        Some(video.saturating_sub(audio))
    }

    pub fn drift_ms(self) -> Option<i64> {
        self.drift_us().map(|value| value / 1_000)
    }
}

/// Thread-safe-by-ownership diagnostics. Keep one instance per live pipeline.
#[derive(Debug, Default, Clone)]
pub struct AvSyncMonitor {
    last_video_pts_us: Option<i64>,
    last_audio_pts_us: Option<i64>,
    video_frames: u64,
    audio_samples: u64,
}

impl AvSyncMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a normalized video timestamp. Negative timestamps are rejected.
    pub fn record_video(&mut self, pts_us: i64) -> bool {
        if pts_us < 0 {
            return false;
        }
        self.last_video_pts_us = Some(pts_us);
        self.video_frames = self.video_frames.saturating_add(1);
        true
    }

    /// Records an audio timestamp and sample count. Negative timestamps and
    /// negative sample counts are rejected without mutating the monitor.
    pub fn record_audio(&mut self, pts_us: i64, samples: i64) -> bool {
        if pts_us < 0 || samples < 0 {
            return false;
        }
        self.last_audio_pts_us = Some(pts_us);
        self.audio_samples = self.audio_samples.saturating_add(samples as u64);
        true
    }

    pub fn snapshot(&self) -> AvSyncSnapshot {
        AvSyncSnapshot {
            last_video_pts_us: self.last_video_pts_us,
            last_audio_pts_us: self.last_audio_pts_us,
            video_frames: self.video_frames,
            audio_samples: self.audio_samples,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Converts incoming timestamps to a non-negative, strictly increasing
/// microsecond timeline. Use per output/track; do not share between audio/video.
#[derive(Debug, Default, Clone)]
pub struct MonotonicPts {
    last_pts_us: Option<i64>,
}

impl MonotonicPts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the supplied timestamp if it is valid and strictly increasing.
    /// Repeated, negative, or backwards timestamps are advanced by one
    /// microsecond from the previous accepted value. The first invalid
    /// timestamp is clamped to zero.
    pub fn normalize(&mut self, pts_us: i64) -> i64 {
        let candidate = pts_us.max(0);
        let normalized = match self.last_pts_us {
            Some(last) if candidate <= last => last.saturating_add(1),
            _ => candidate,
        };
        self.last_pts_us = Some(normalized);
        normalized
    }

    pub fn last(&self) -> Option<i64> {
        self.last_pts_us
    }

    pub fn reset(&mut self) {
        self.last_pts_us = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_reports_signed_video_minus_audio_drift() {
        let mut monitor = AvSyncMonitor::new();
        assert_eq!(monitor.snapshot().drift_us(), None);
        assert!(monitor.record_video(40_000));
        assert!(monitor.record_audio(32_000, 480));
        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.drift_us(), Some(8_000));
        assert_eq!(snapshot.drift_ms(), Some(8));
        assert_eq!(snapshot.video_frames, 1);
        assert_eq!(snapshot.audio_samples, 480);
    }

    #[test]
    fn monitor_preserves_negative_drift_and_rejects_invalid_input() {
        let mut monitor = AvSyncMonitor::new();
        assert!(monitor.record_video(10_000));
        assert!(monitor.record_audio(18_000, 256));
        assert_eq!(monitor.snapshot().drift_us(), Some(-8_000));
        let before = monitor.snapshot();
        assert!(!monitor.record_video(-1));
        assert!(!monitor.record_audio(-1, 10));
        assert!(!monitor.record_audio(10, -10));
        assert_eq!(monitor.snapshot(), before);
    }

    #[test]
    fn pts_normalizer_repairs_repeated_and_backwards_timestamps() {
        let mut pts = MonotonicPts::new();
        assert_eq!(pts.normalize(10), 10);
        assert_eq!(pts.normalize(10), 11);
        assert_eq!(pts.normalize(4), 12);
        assert_eq!(pts.normalize(-1), 13);
        assert_eq!(pts.last(), Some(13));
        pts.reset();
        assert_eq!(pts.normalize(-1), 0);
    }

    #[test]
    fn pts_normalizer_saturates_at_integer_limit() {
        let mut pts = MonotonicPts::new();
        assert_eq!(pts.normalize(i64::MAX), i64::MAX);
        assert_eq!(pts.normalize(i64::MAX), i64::MAX);
    }

    #[test]
    fn monitor_reset_clears_timestamps_and_counters() {
        let mut monitor = AvSyncMonitor::new();
        monitor.record_video(100);
        monitor.record_audio(90, 20);
        monitor.reset();
        assert_eq!(monitor.snapshot(), AvSyncSnapshot {
            last_video_pts_us: None,
            last_audio_pts_us: None,
            video_frames: 0,
            audio_samples: 0,
        });
    }
}
