//! Typed, validated media packets for the synchronization pipeline.
//!
//! Packet timestamps use the same session-relative microsecond timebase as
//! the A/V diagnostics. This module stores metadata only; it does not own or
//! decode the media payload.

/// Media stream represented by a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    Audio,
    Video,
}

/// Metadata and payload for one audio or video unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPacket<T> {
    kind: MediaKind,
    pts_us: i64,
    duration_us: Option<i64>,
    sequence: u64,
    payload: T,
}

/// Invalid timestamp or duration supplied to a packet constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaPacketError {
    NegativePts,
    NonPositiveDuration,
}

impl<T> MediaPacket<T> {
    /// Creates a packet using session-relative microseconds.
    ///
    /// Duration may be omitted when it is not yet known. If present, it must
    /// be positive. The sequence number is local to the producer/track.
    pub fn new(
        kind: MediaKind,
        pts_us: i64,
        duration_us: Option<i64>,
        sequence: u64,
        payload: T,
    ) -> Result<Self, MediaPacketError> {
        if pts_us < 0 {
            return Err(MediaPacketError::NegativePts);
        }
        if duration_us.is_some_and(|duration| duration <= 0) {
            return Err(MediaPacketError::NonPositiveDuration);
        }
        Ok(Self {
            kind,
            pts_us,
            duration_us,
            sequence,
            payload,
        })
    }

    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    pub fn pts_us(&self) -> i64 {
        self.pts_us
    }

    pub fn duration_us(&self) -> Option<i64> {
        self.duration_us
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut T {
        &mut self.payload
    }

    pub fn into_payload(self) -> T {
        self.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_audio_packet_with_valid_timing_metadata() {
        let packet =
            MediaPacket::new(MediaKind::Audio, 20_000, Some(10_000), 7, vec![1, 2]).unwrap();
        assert_eq!(packet.kind(), MediaKind::Audio);
        assert_eq!(packet.pts_us(), 20_000);
        assert_eq!(packet.duration_us(), Some(10_000));
        assert_eq!(packet.sequence(), 7);
        assert_eq!(packet.payload(), &vec![1, 2]);
    }

    #[test]
    fn allows_unknown_duration() {
        let packet = MediaPacket::new(MediaKind::Video, 0, None, 0, "frame").unwrap();
        assert_eq!(packet.duration_us(), None);
        assert_eq!(packet.into_payload(), "frame");
    }

    #[test]
    fn rejects_negative_pts_and_non_positive_durations() {
        assert_eq!(
            MediaPacket::new(MediaKind::Audio, -1, None, 0, ()),
            Err(MediaPacketError::NegativePts)
        );
        assert_eq!(
            MediaPacket::new(MediaKind::Video, 0, Some(0), 0, ()),
            Err(MediaPacketError::NonPositiveDuration)
        );
        assert_eq!(
            MediaPacket::new(MediaKind::Video, 0, Some(-1), 0, ()),
            Err(MediaPacketError::NonPositiveDuration)
        );
    }

    #[test]
    fn payload_can_be_mutated_or_taken_without_copying() {
        let mut packet =
            MediaPacket::new(MediaKind::Video, 33_333, Some(33_333), 1, vec![1]).unwrap();
        packet.payload_mut().push(2);
        assert_eq!(packet.into_payload(), vec![1, 2]);
    }
}
