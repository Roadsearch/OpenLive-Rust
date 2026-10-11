//! Stream configuration ported from `core/model/StreamModel.kt`.

use crate::thermal::ThermalLevel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamConfig {
    pub rtmp_url: String,
    pub stream_key: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub video_bitrate_kbps: u32,
    pub audio_bitrate_kbps: u32,
    pub keyframe_interval_sec: u32,
    pub extra_destinations: Vec<String>,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            rtmp_url: "rtmp://live.twitch.tv/app/".into(),
            stream_key: String::new(),
            width: 1920,
            height: 1080,
            fps: 30,
            video_bitrate_kbps: 4500,
            audio_bitrate_kbps: 128,
            keyframe_interval_sec: 2,
            extra_destinations: Vec::new(),
        }
    }
}

impl StreamConfig {
    /// Joins the configured RTMP base URL and stream key using the Kotlin rule.
    pub fn full_rtmp_url(&self) -> String {
        if self.rtmp_url.ends_with('/') {
            format!("{}{}", self.rtmp_url, self.stream_key)
        } else {
            format!("{}/{}", self.rtmp_url, self.stream_key)
        }
    }

    /// Preserves OpenLive's thermal bitrate policy, including the 500 kbps floor.
    pub fn effective_video_bitrate_kbps(&self, level: ThermalLevel) -> u32 {
        let factor = if level == ThermalLevel::Emergency {
            1.0
        } else {
            level.bitrate_factor()
        };
        ((self.video_bitrate_kbps as f32 * factor) as u32).max(500)
    }

    /// Extra clients are started only for non-empty RTMP/RTMPS URLs.
    pub fn valid_extra_destinations(&self) -> impl Iterator<Item = &str> {
        self.extra_destinations
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .filter(|url| {
                url.get(..7)
                    .is_some_and(|scheme| scheme.eq_ignore_ascii_case("rtmp://"))
                    || url
                        .get(..8)
                        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("rtmps://"))
            })
    }
}
