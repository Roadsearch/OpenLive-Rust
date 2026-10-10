//! Per-track packet ordering diagnostics for live audio/video pipelines.
//!
//! This tracker does not reorder, drop, buffer, or play packets. It records
//! timestamp order independently for audio and video so a pipeline can make
//! an explicit scheduling decision without allowing a late packet to move a
//! track's high-water mark backwards.

use crate::media_packet::{MediaKind, MediaPacket};

/// Classification of a packet's PTS relative to the same media track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketOrder {
    /// First timestamp observed for this track.
    First,
    /// PTS is strictly greater than the previous high-water mark.
    InOrder,
    /// PTS equals the previous high-water mark.
    RepeatedPts,
    /// PTS is lower than the previous high-water mark.
    OutOfOrder,
}

/// Statistics for one media track.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TrackOrderStats {
    /// Greatest PTS accepted as the track's high-water mark.
    pub high_watermark_pts_us: Option<i64>,
    pub observed_packets: u64,
    pub in_order_packets: u64,
    pub repeated_pts_packets: u64,
    pub out_of_order_packets: u64,
}

impl TrackOrderStats {
    /// Number of packets whose timestamps did not advance the high-water mark.
    pub fn non_advancing_packets(self) -> u64 {
        self.repeated_pts_packets
            .saturating_add(self.out_of_order_packets)
    }
}

/// Snapshot of audio/video ordering and their latest high-water-mark skew.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PacketOrderSnapshot {
    pub audio: TrackOrderStats,
    pub video: TrackOrderStats,
}

impl PacketOrderSnapshot {
    /// Signed video-minus-audio skew in microseconds, if both tracks were seen.
    pub fn av_skew_us(self) -> Option<i64> {
        Some(
            self.video
                .high_watermark_pts_us?
                .saturating_sub(self.audio.high_watermark_pts_us?),
        )
    }
}

/// Tracks timestamp progression separately for audio and video.
#[derive(Debug, Default, Clone)]
pub struct PacketOrderTracker {
    audio: TrackOrderStats,
    video: TrackOrderStats,
}

impl PacketOrderTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observes a validated packet and classifies its PTS against its own track.
    ///
    /// Repeated and out-of-order packets are counted but do not lower the
    /// high-water mark. The caller retains ownership of the packet and decides
    /// whether to buffer, reject, or otherwise handle it.
    pub fn observe<T>(&mut self, packet: &MediaPacket<T>) -> PacketOrder {
        let stats = match packet.kind() {
            MediaKind::Audio => &mut self.audio,
            MediaKind::Video => &mut self.video,
        };
        stats.observed_packets = stats.observed_packets.saturating_add(1);

        let order = match stats.high_watermark_pts_us {
            None => PacketOrder::First,
            Some(previous) if packet.pts_us() > previous => PacketOrder::InOrder,
            Some(previous) if packet.pts_us() == previous => PacketOrder::RepeatedPts,
            Some(_) => PacketOrder::OutOfOrder,
        };

        match order {
            PacketOrder::First | PacketOrder::InOrder => {
                stats.high_watermark_pts_us = Some(packet.pts_us());
                stats.in_order_packets = stats.in_order_packets.saturating_add(1);
            }
            PacketOrder::RepeatedPts => {
                stats.repeated_pts_packets = stats.repeated_pts_packets.saturating_add(1);
            }
            PacketOrder::OutOfOrder => {
                stats.out_of_order_packets = stats.out_of_order_packets.saturating_add(1);
            }
        }
        order
    }

    pub fn snapshot(&self) -> PacketOrderSnapshot {
        PacketOrderSnapshot {
            audio: self.audio,
            video: self.video,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(kind: MediaKind, pts_us: i64, sequence: u64) -> MediaPacket<()> {
        MediaPacket::new(kind, pts_us, None, sequence, ()).unwrap()
    }

    #[test]
    fn tracks_audio_and_video_order_independently() {
        let mut tracker = PacketOrderTracker::new();
        assert_eq!(tracker.observe(&packet(MediaKind::Video, 33_000, 0)), PacketOrder::First);
        assert_eq!(tracker.observe(&packet(MediaKind::Audio, 10_000, 0)), PacketOrder::First);
        assert_eq!(tracker.observe(&packet(MediaKind::Audio, 20_000, 1)), PacketOrder::InOrder);
        assert_eq!(tracker.observe(&packet(MediaKind::Video, 66_000, 1)), PacketOrder::InOrder);

        let snapshot = tracker.snapshot();
        assert_eq!(snapshot.audio.high_watermark_pts_us, Some(20_000));
        assert_eq!(snapshot.video.high_watermark_pts_us, Some(66_000));
        assert_eq!(snapshot.av_skew_us(), Some(46_000));
    }

    #[test]
    fn repeated_and_backwards_pts_do_not_lower_watermark() {
        let mut tracker = PacketOrderTracker::new();
        tracker.observe(&packet(MediaKind::Audio, 100, 0));
        assert_eq!(tracker.observe(&packet(MediaKind::Audio, 100, 1)), PacketOrder::RepeatedPts);
        assert_eq!(tracker.observe(&packet(MediaKind::Audio, 90, 2)), PacketOrder::OutOfOrder);
        let stats = tracker.snapshot().audio;
        assert_eq!(stats.high_watermark_pts_us, Some(100));
        assert_eq!(stats.observed_packets, 3);
        assert_eq!(stats.in_order_packets, 1);
        assert_eq!(stats.repeated_pts_packets, 1);
        assert_eq!(stats.out_of_order_packets, 1);
        assert_eq!(stats.non_advancing_packets(), 2);
    }

    #[test]
    fn skew_is_unavailable_until_both_tracks_are_seen() {
        let mut tracker = PacketOrderTracker::new();
        assert_eq!(tracker.snapshot().av_skew_us(), None);
        tracker.observe(&packet(MediaKind::Video, 500, 0));
        assert_eq!(tracker.snapshot().av_skew_us(), None);
        tracker.observe(&packet(MediaKind::Audio, 800, 0));
        assert_eq!(tracker.snapshot().av_skew_us(), Some(-300));
    }

    #[test]
    fn reset_clears_both_track_counters_and_watermarks() {
        let mut tracker = PacketOrderTracker::new();
        tracker.observe(&packet(MediaKind::Video, 500, 0));
        tracker.observe(&packet(MediaKind::Audio, 800, 0));
        tracker.reset();
        assert_eq!(tracker.snapshot(), PacketOrderSnapshot::default());
    }
}
