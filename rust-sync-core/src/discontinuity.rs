//! Timestamp discontinuity detection for a single media track.
//!
//! This module detects suspicious PTS changes; it does not repair timestamps,
//! flush decoders, or drop packets. Create one detector per track and decide at
//! the pipeline boundary how to recover from a reported discontinuity.

/// Classification of a PTS relative to the previous timestamp on one track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampEvent {
    /// First timestamp observed by this detector.
    First,
    /// Timestamp advanced by less than the configured forward-gap threshold.
    Continuous { delta_us: i64 },
    /// Timestamp repeated the previous value.
    Repeated,
    /// Timestamp advanced by at least the configured forward-gap threshold.
    ForwardGap { delta_us: i64 },
    /// Timestamp moved backwards by less than the discontinuity threshold.
    OutOfOrder { backward_us: i64 },
    /// Timestamp moved backwards by at least the configured threshold.
    BackwardJump { backward_us: i64 },
}

/// Invalid configuration or timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscontinuityConfigError {
    ZeroForwardGapThreshold,
    ZeroBackwardJumpThreshold,
}

/// Returned when a negative PTS is observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegativeTimestampError;

/// Counters collected by a detector.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DiscontinuityStats {
    pub observed: u64,
    pub continuous: u64,
    pub repeated: u64,
    pub forward_gaps: u64,
    pub out_of_order: u64,
    pub backward_jumps: u64,
}

/// Detects large forward gaps and backward jumps in one track's PTS sequence.
#[derive(Debug, Clone)]
pub struct TimestampDiscontinuityDetector {
    max_forward_gap_us: i64,
    max_backward_jump_us: i64,
    last_pts_us: Option<i64>,
    stats: DiscontinuityStats,
}

impl TimestampDiscontinuityDetector {
    /// Thresholds must be positive and use the same microsecond timebase as PTS.
    pub fn new(
        max_forward_gap_us: i64,
        max_backward_jump_us: i64,
    ) -> Result<Self, DiscontinuityConfigError> {
        if max_forward_gap_us <= 0 {
            return Err(DiscontinuityConfigError::ZeroForwardGapThreshold);
        }
        if max_backward_jump_us <= 0 {
            return Err(DiscontinuityConfigError::ZeroBackwardJumpThreshold);
        }
        Ok(Self {
            max_forward_gap_us,
            max_backward_jump_us,
            last_pts_us: None,
            stats: DiscontinuityStats::default(),
        })
    }

    /// Classifies a timestamp and updates the previous-observation baseline.
    ///
    /// Even an out-of-order timestamp becomes the next comparison baseline.
    /// Use a separate order tracker if a high-water mark is also needed.
    pub fn observe_pts(
        &mut self,
        pts_us: i64,
    ) -> Result<TimestampEvent, NegativeTimestampError> {
        if pts_us < 0 {
            return Err(NegativeTimestampError);
        }
        self.stats.observed = self.stats.observed.saturating_add(1);
        let event = match self.last_pts_us {
            None => TimestampEvent::First,
            Some(previous) if pts_us == previous => TimestampEvent::Repeated,
            Some(previous) if pts_us > previous => {
                let delta_us = pts_us.saturating_sub(previous);
                if delta_us >= self.max_forward_gap_us {
                    TimestampEvent::ForwardGap { delta_us }
                } else {
                    TimestampEvent::Continuous { delta_us }
                }
            }
            Some(previous) => {
                let backward_us = previous.saturating_sub(pts_us);
                if backward_us >= self.max_backward_jump_us {
                    TimestampEvent::BackwardJump { backward_us }
                } else {
                    TimestampEvent::OutOfOrder { backward_us }
                }
            }
        };
        match event {
            TimestampEvent::First | TimestampEvent::Continuous { .. } => {
                self.stats.continuous = self.stats.continuous.saturating_add(1);
            }
            TimestampEvent::Repeated => {
                self.stats.repeated = self.stats.repeated.saturating_add(1);
            }
            TimestampEvent::ForwardGap { .. } => {
                self.stats.forward_gaps = self.stats.forward_gaps.saturating_add(1);
            }
            TimestampEvent::OutOfOrder { .. } => {
                self.stats.out_of_order = self.stats.out_of_order.saturating_add(1);
            }
            TimestampEvent::BackwardJump { .. } => {
                self.stats.backward_jumps = self.stats.backward_jumps.saturating_add(1);
            }
        }
        self.last_pts_us = Some(pts_us);
        Ok(event)
    }

    pub fn last_pts_us(&self) -> Option<i64> {
        self.last_pts_us
    }

    pub fn stats(&self) -> DiscontinuityStats {
        self.stats
    }

    /// Clears the baseline and counters, for example at a new stream epoch.
    pub fn reset(&mut self) {
        self.last_pts_us = None;
        self.stats = DiscontinuityStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> TimestampDiscontinuityDetector {
        TimestampDiscontinuityDetector::new(100_000, 50_000).unwrap()
    }

    #[test]
    fn classifies_first_continuous_and_repeated_timestamps() {
        let mut detector = detector();
        assert_eq!(detector.observe_pts(0), Ok(TimestampEvent::First));
        assert_eq!(
            detector.observe_pts(33_333),
            Ok(TimestampEvent::Continuous { delta_us: 33_333 })
        );
        assert_eq!(detector.observe_pts(33_333), Ok(TimestampEvent::Repeated));
        assert_eq!(detector.stats().observed, 3);
    }

    #[test]
    fn detects_large_forward_gap_and_backward_jump() {
        let mut detector = detector();
        detector.observe_pts(100_000).unwrap();
        assert_eq!(
            detector.observe_pts(250_000),
            Ok(TimestampEvent::ForwardGap { delta_us: 150_000 })
        );
        assert_eq!(
            detector.observe_pts(150_000),
            Ok(TimestampEvent::BackwardJump { backward_us: 100_000 })
        );
        let stats = detector.stats();
        assert_eq!(stats.forward_gaps, 1);
        assert_eq!(stats.backward_jumps, 1);
    }

    #[test]
    fn small_backward_change_is_out_of_order_and_becomes_new_baseline() {
        let mut detector = detector();
        detector.observe_pts(20_000).unwrap();
        assert_eq!(
            detector.observe_pts(10_000),
            Ok(TimestampEvent::OutOfOrder { backward_us: 10_000 })
        );
        assert_eq!(detector.last_pts_us(), Some(10_000));
    }

    #[test]
    fn rejects_negative_timestamps_without_mutating_stats() {
        let mut detector = detector();
        assert_eq!(detector.observe_pts(-1), Err(NegativeTimestampError));
        assert_eq!(detector.stats(), DiscontinuityStats::default());
        assert_eq!(detector.last_pts_us(), None);
    }

    #[test]
    fn rejects_zero_thresholds() {
        assert_eq!(
            TimestampDiscontinuityDetector::new(0, 1).err(),
            Some(DiscontinuityConfigError::ZeroForwardGapThreshold)
        );
        assert_eq!(
            TimestampDiscontinuityDetector::new(1, 0).err(),
            Some(DiscontinuityConfigError::ZeroBackwardJumpThreshold)
        );
    }

    #[test]
    fn reset_starts_a_new_timestamp_epoch() {
        let mut detector = detector();
        detector.observe_pts(1_000).unwrap();
        detector.reset();
        assert_eq!(detector.last_pts_us(), None);
        assert_eq!(detector.observe_pts(0), Ok(TimestampEvent::First));
        assert_eq!(detector.stats().observed, 1);
    }
}
