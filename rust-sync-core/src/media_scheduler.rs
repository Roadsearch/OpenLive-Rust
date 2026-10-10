//! Bounded, timestamp-ordered A/V packet scheduler.
//!
//! Packets are buffered independently per track and emitted by earliest PTS.
//! This is a deterministic queue primitive, not a wall-clock playback loop:
//! callers remain responsible for waiting until a packet is due and for audio
//! device / decoder back-pressure.

use std::collections::VecDeque;

use crate::bounded_queue::OverflowPolicy;
use crate::media_packet::{MediaKind, MediaPacket};

/// Result of inserting a packet into a bounded timestamp-ordered track queue.
#[derive(Debug, PartialEq, Eq)]
pub enum SchedulerPushResult<T> {
    Queued,
    /// The incoming packet was rejected because the track queue was full.
    DroppedIncoming(MediaPacket<T>),
    /// The incoming packet was queued after evicting the earliest-PTS packet.
    QueuedEvicting(MediaPacket<T>),
}

/// Configuration error for a scheduler with an unusable queue capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerCapacityError;

/// Current queue occupancy and overload counters for a track.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrackQueueStats {
    pub capacity: usize,
    pub queued_packets: usize,
    pub dropped_packets: u64,
}

/// Queue-level snapshot for both tracks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MediaSchedulerSnapshot {
    pub audio: TrackQueueStats,
    pub video: TrackQueueStats,
    pub emitted_packets: u64,
}

/// Bounded A/V scheduler that preserves PTS ordering within each track.
#[derive(Debug, Clone)]
pub struct MediaScheduler<T> {
    audio: VecDeque<MediaPacket<T>>,
    video: VecDeque<MediaPacket<T>>,
    audio_capacity: usize,
    video_capacity: usize,
    audio_policy: OverflowPolicy,
    video_policy: OverflowPolicy,
    audio_dropped: u64,
    video_dropped: u64,
    emitted_packets: u64,
}

impl<T> MediaScheduler<T> {
    pub fn new(
        audio_capacity: usize,
        video_capacity: usize,
        audio_policy: OverflowPolicy,
        video_policy: OverflowPolicy,
    ) -> Result<Self, SchedulerCapacityError> {
        if audio_capacity == 0 || video_capacity == 0 {
            return Err(SchedulerCapacityError);
        }
        Ok(Self {
            audio: VecDeque::with_capacity(audio_capacity),
            video: VecDeque::with_capacity(video_capacity),
            audio_capacity,
            video_capacity,
            audio_policy,
            video_policy,
            audio_dropped: 0,
            video_dropped: 0,
            emitted_packets: 0,
        })
    }

    /// Inserts a packet into its track in ascending (PTS, sequence) order.
    ///
    /// If full, the configured policy applies. DropOldest evicts the packet
    /// with the smallest PTS, not the packet inserted earliest in wall-clock
    /// time. DropNewest rejects the incoming packet.
    pub fn push(&mut self, packet: MediaPacket<T>) -> SchedulerPushResult<T> {
        let kind = packet.kind();
        let (queue, capacity, policy, dropped_count) = match kind {
            MediaKind::Audio => (
                &mut self.audio,
                self.audio_capacity,
                self.audio_policy,
                &mut self.audio_dropped,
            ),
            MediaKind::Video => (
                &mut self.video,
                self.video_capacity,
                self.video_policy,
                &mut self.video_dropped,
            ),
        };

        let evicted = if queue.len() >= capacity {
            *dropped_count = (*dropped_count).saturating_add(1);
            match policy {
                OverflowPolicy::DropNewest => return SchedulerPushResult::DroppedIncoming(packet),
                OverflowPolicy::DropOldest => queue.pop_front(),
            }
        } else {
            None
        };

        let key_pts = packet.pts_us();
        let key_sequence = packet.sequence();
        let insertion_index = queue
            .iter()
            .position(|queued| (queued.pts_us(), queued.sequence()) > (key_pts, key_sequence))
            .unwrap_or(queue.len());
        queue.insert(insertion_index, packet);

        match evicted {
            Some(packet) => SchedulerPushResult::QueuedEvicting(packet),
            None => SchedulerPushResult::Queued,
        }
    }

    /// Removes the queued packet with the earliest PTS across both tracks.
    /// Equal PTS values prefer audio, then lower sequence within each track.
    pub fn pop_next(&mut self) -> Option<MediaPacket<T>> {
        let audio_key = self.audio.front().map(|p| (p.pts_us(), 0_u8, p.sequence()));
        let video_key = self.video.front().map(|p| (p.pts_us(), 1_u8, p.sequence()));

        let packet = match (audio_key, video_key) {
            (None, None) => return None,
            (Some(_), None) => self.audio.pop_front(),
            (None, Some(_)) => self.video.pop_front(),
            (Some(audio), Some(video)) if audio <= video => self.audio.pop_front(),
            (Some(_), Some(_)) => self.video.pop_front(),
        };
        if packet.is_some() {
            self.emitted_packets = self.emitted_packets.saturating_add(1);
        }
        packet
    }

    pub fn snapshot(&self) -> MediaSchedulerSnapshot {
        MediaSchedulerSnapshot {
            audio: TrackQueueStats {
                capacity: self.audio_capacity,
                queued_packets: self.audio.len(),
                dropped_packets: self.audio_dropped,
            },
            video: TrackQueueStats {
                capacity: self.video_capacity,
                queued_packets: self.video.len(),
                dropped_packets: self.video_dropped,
            },
            emitted_packets: self.emitted_packets,
        }
    }

    /// Returns the earliest queued presentation timestamp without consuming a packet.
    pub fn peek_next_pts_us(&self) -> Option<i64> {
        match (self.audio.front(), self.video.front()) {
            (None, None) => None,
            (Some(packet), None) | (None, Some(packet)) => Some(packet.pts_us()),
            (Some(audio), Some(video)) => Some(audio.pts_us().min(video.pts_us())),
        }
    }

    /// Reports whether the selected track has reached its configured capacity.
    pub fn is_full(&self, kind: MediaKind) -> bool {
        match kind {
            MediaKind::Audio => self.audio.len() >= self.audio_capacity,
            MediaKind::Video => self.video.len() >= self.video_capacity,
        }
    }

    /// Clears pending packets but retains counters for diagnostics.
    pub fn clear(&mut self) {
        self.audio.clear();
        self.video.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.video.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(
        kind: MediaKind,
        pts_us: i64,
        sequence: u64,
        payload: &'static str,
    ) -> MediaPacket<&'static str> {
        MediaPacket::new(kind, pts_us, None, sequence, payload).unwrap()
    }

    fn scheduler(
        audio_capacity: usize,
        video_capacity: usize,
        audio_policy: OverflowPolicy,
        video_policy: OverflowPolicy,
    ) -> MediaScheduler<&'static str> {
        MediaScheduler::new(audio_capacity, video_capacity, audio_policy, video_policy).unwrap()
    }

    #[test]
    fn rejects_zero_capacity_for_either_track() {
        assert!(MediaScheduler::<()>::new(
            0,
            1,
            OverflowPolicy::DropNewest,
            OverflowPolicy::DropOldest
        )
        .is_err());
        assert!(MediaScheduler::<()>::new(
            1,
            0,
            OverflowPolicy::DropNewest,
            OverflowPolicy::DropOldest
        )
        .is_err());
    }

    #[test]
    fn emits_global_earliest_pts_even_when_arrivals_are_out_of_order() {
        let mut scheduler = scheduler(4, 4, OverflowPolicy::DropNewest, OverflowPolicy::DropNewest);
        assert_eq!(
            scheduler.push(packet(MediaKind::Video, 30, 0, "v30")),
            SchedulerPushResult::Queued
        );
        assert_eq!(
            scheduler.push(packet(MediaKind::Audio, 10, 0, "a10")),
            SchedulerPushResult::Queued
        );
        assert_eq!(
            scheduler.push(packet(MediaKind::Audio, 5, 1, "a5")),
            SchedulerPushResult::Queued
        );
        assert_eq!(scheduler.pop_next().unwrap().payload(), &"a5");
        assert_eq!(scheduler.pop_next().unwrap().payload(), &"a10");
        assert_eq!(scheduler.pop_next().unwrap().payload(), &"v30");
        assert!(scheduler.pop_next().is_none());
        assert_eq!(scheduler.snapshot().emitted_packets, 3);
    }

    #[test]
    fn equal_pts_prefers_audio() {
        let mut scheduler = scheduler(2, 2, OverflowPolicy::DropNewest, OverflowPolicy::DropNewest);
        scheduler.push(packet(MediaKind::Video, 10, 0, "video"));
        scheduler.push(packet(MediaKind::Audio, 10, 0, "audio"));
        assert_eq!(scheduler.pop_next().unwrap().kind(), MediaKind::Audio);
        assert_eq!(scheduler.pop_next().unwrap().kind(), MediaKind::Video);
    }

    #[test]
    fn drop_newest_rejects_incoming_packet_when_track_is_full() {
        let mut scheduler = scheduler(1, 1, OverflowPolicy::DropNewest, OverflowPolicy::DropOldest);
        scheduler.push(packet(MediaKind::Audio, 10, 0, "kept"));
        assert_eq!(
            scheduler.push(packet(MediaKind::Audio, 5, 1, "rejected")),
            SchedulerPushResult::DroppedIncoming(packet(MediaKind::Audio, 5, 1, "rejected"))
        );
        assert_eq!(scheduler.pop_next().unwrap().payload(), &"kept");
        assert_eq!(scheduler.snapshot().audio.dropped_packets, 1);
    }

    #[test]
    fn drop_oldest_evicts_smallest_pts_not_oldest_arrival() {
        let mut scheduler = scheduler(2, 2, OverflowPolicy::DropNewest, OverflowPolicy::DropOldest);
        scheduler.push(packet(MediaKind::Video, 30, 0, "v30"));
        scheduler.push(packet(MediaKind::Video, 10, 1, "v10"));
        assert_eq!(
            scheduler.push(packet(MediaKind::Video, 20, 2, "v20")),
            SchedulerPushResult::QueuedEvicting(packet(MediaKind::Video, 10, 1, "v10"))
        );
        assert_eq!(scheduler.pop_next().unwrap().payload(), &"v20");
        assert_eq!(scheduler.pop_next().unwrap().payload(), &"v30");
        assert_eq!(scheduler.snapshot().video.dropped_packets, 1);
    }

    #[test]
    fn peek_reports_earliest_pts_without_consuming_packet() {
        let mut scheduler = scheduler(2, 2, OverflowPolicy::DropNewest, OverflowPolicy::DropNewest);
        assert_eq!(scheduler.peek_next_pts_us(), None);
        scheduler.push(packet(MediaKind::Video, 30, 0, "v30"));
        scheduler.push(packet(MediaKind::Audio, 10, 0, "a10"));
        assert_eq!(scheduler.peek_next_pts_us(), Some(10));
        assert_eq!(scheduler.snapshot().audio.queued_packets, 1);
        assert_eq!(scheduler.pop_next().unwrap().payload(), &"a10");
        assert_eq!(scheduler.peek_next_pts_us(), Some(30));
    }

    #[test]
    fn full_status_is_track_specific() {
        let mut scheduler = scheduler(1, 2, OverflowPolicy::DropNewest, OverflowPolicy::DropNewest);
        assert!(!scheduler.is_full(MediaKind::Audio));
        assert!(!scheduler.is_full(MediaKind::Video));
        scheduler.push(packet(MediaKind::Audio, 10, 0, "a10"));
        assert!(scheduler.is_full(MediaKind::Audio));
        assert!(!scheduler.is_full(MediaKind::Video));
    }

    #[test]
    fn clear_keeps_drop_counters_and_resets_occupancy() {
        let mut scheduler = scheduler(1, 1, OverflowPolicy::DropNewest, OverflowPolicy::DropNewest);
        scheduler.push(packet(MediaKind::Audio, 1, 0, "one"));
        scheduler.push(packet(MediaKind::Audio, 2, 1, "dropped"));
        scheduler.clear();
        assert!(scheduler.is_empty());
        assert_eq!(scheduler.snapshot().audio.queued_packets, 0);
        assert_eq!(scheduler.snapshot().audio.dropped_packets, 1);
    }
}
