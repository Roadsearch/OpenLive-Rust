//! Output lifecycle policy ported from `core/pipeline/LivePipeline.kt`.
//!
//! The Android adapter performs the effects (starting MediaCodec, AudioRecord,
//! RTMP clients and MediaMuxer). This controller decides when shared media
//! infrastructure and the session clock need to start or can safely stop.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Idle,
    Connecting,
    Live,
    Reconnecting,
}

impl StreamState {
    fn active(self) -> bool {
        matches!(self, Self::Connecting | Self::Live | Self::Reconnecting)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordingState {
    #[default]
    Idle,
    Recording,
    Paused,
}

impl RecordingState {
    fn active(self) -> bool {
        self != Self::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Stream,
    Recording,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineError {
    EncoderUnavailable,
    OutputNotActive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineEffect {
    StartForegroundService,
    StartSessionClock,
    StartAudioMixer,
    ConnectPrimaryStream,
    StartExtraStreams,
    DisconnectStreams,
    StopExtraStreams,
    StartMp4Recording,
    PauseMp4Recording,
    ResumeMp4Recording,
    RequestVideoKeyFrame,
    FinalizeMp4Recording,
    StopVideoEncoder,
    StopAudioEncoder,
    StopAudioMixer,
    StopSessionClock,
    StopForegroundService,
    DetachEncoderSurface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PipelineSnapshot {
    pub stream: Option<StreamState>,
    pub recording: RecordingState,
    pub media_infrastructure_running: bool,
}

#[derive(Debug, Default)]
pub struct PipelineController {
    stream: Option<StreamState>,
    recording: RecordingState,
    media_infrastructure_running: bool,
}

impl PipelineController {
    /// Starts or reconnects streaming. The host must ensure both encoders exist
    /// before passing `encoders_ready = true`, as `LivePipeline.kt` does.
    pub fn start_streaming(
        &mut self,
        encoders_ready: bool,
    ) -> Result<Vec<PipelineEffect>, PipelineError> {
        if !encoders_ready {
            return Err(PipelineError::EncoderUnavailable);
        }
        if self.stream.is_some_and(StreamState::active) {
            return Ok(Vec::new());
        }
        let mut effects = self.start_shared_infrastructure();
        self.stream = Some(StreamState::Connecting);
        effects.extend([
            PipelineEffect::ConnectPrimaryStream,
            PipelineEffect::StartExtraStreams,
            PipelineEffect::StartAudioMixer,
        ]);
        Ok(effects)
    }

    pub fn start_recording(
        &mut self,
        encoders_ready: bool,
    ) -> Result<Vec<PipelineEffect>, PipelineError> {
        if !encoders_ready {
            return Err(PipelineError::EncoderUnavailable);
        }
        if self.recording == RecordingState::Recording {
            return Ok(Vec::new());
        }
        if self.recording == RecordingState::Paused {
            return self.resume_recording();
        }
        let mut effects = self.start_shared_infrastructure();
        self.recording = RecordingState::Recording;
        effects.extend([
            PipelineEffect::StartMp4Recording,
            PipelineEffect::RequestVideoKeyFrame,
            PipelineEffect::StartAudioMixer,
        ]);
        Ok(effects)
    }

    pub fn pause_recording(&mut self) -> Result<Vec<PipelineEffect>, PipelineError> {
        if self.recording != RecordingState::Recording {
            return Err(PipelineError::OutputNotActive);
        }
        self.recording = RecordingState::Paused;
        Ok(vec![PipelineEffect::PauseMp4Recording])
    }

    pub fn resume_recording(&mut self) -> Result<Vec<PipelineEffect>, PipelineError> {
        if self.recording != RecordingState::Paused {
            return Err(PipelineError::OutputNotActive);
        }
        self.recording = RecordingState::Recording;
        Ok(vec![
            PipelineEffect::ResumeMp4Recording,
            PipelineEffect::RequestVideoKeyFrame,
        ])
    }

    pub fn stop_streaming(&mut self) -> Vec<PipelineEffect> {
        self.stream = None;
        let mut effects = vec![
            PipelineEffect::DisconnectStreams,
            PipelineEffect::StopExtraStreams,
        ];
        effects.extend(self.stop_shared_infrastructure_if_idle());
        effects
    }

    pub fn stop_recording(&mut self) -> Vec<PipelineEffect> {
        self.recording = RecordingState::Idle;
        let mut effects = vec![PipelineEffect::FinalizeMp4Recording];
        effects.extend(self.stop_shared_infrastructure_if_idle());
        effects
    }

    /// Reconcile a transport callback. Failed/idle streams no longer keep the
    /// encoders or clock alive unless recording is still active.
    pub fn set_stream_state(&mut self, state: StreamState) -> Vec<PipelineEffect> {
        self.stream = if state == StreamState::Idle {
            None
        } else {
            Some(state)
        };
        self.stop_shared_infrastructure_if_idle()
    }

    pub fn snapshot(&self) -> PipelineSnapshot {
        PipelineSnapshot {
            stream: self.stream,
            recording: self.recording,
            media_infrastructure_running: self.media_infrastructure_running,
        }
    }

    fn start_shared_infrastructure(&mut self) -> Vec<PipelineEffect> {
        if self.media_infrastructure_running {
            return Vec::new();
        }
        self.media_infrastructure_running = true;
        vec![
            PipelineEffect::StartForegroundService,
            PipelineEffect::StartSessionClock,
        ]
    }

    fn stop_shared_infrastructure_if_idle(&mut self) -> Vec<PipelineEffect> {
        let stream_active = self.stream.is_some_and(StreamState::active);
        if stream_active || self.recording.active() || !self.media_infrastructure_running {
            return Vec::new();
        }
        self.media_infrastructure_running = false;
        vec![
            PipelineEffect::DetachEncoderSurface,
            PipelineEffect::StopVideoEncoder,
            PipelineEffect::StopAudioEncoder,
            PipelineEffect::StopAudioMixer,
            PipelineEffect::StopSessionClock,
            PipelineEffect::StopForegroundService,
        ]
    }
}
