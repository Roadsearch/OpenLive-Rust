package com.example.core.audio

/** Small JNI owner for Rust's stateful PCM microphone processor. */
class RustAudioProcessor : AutoCloseable {
    private var handle: Long = nativeCreate()

    /**
     * Processes an interleaved stereo ShortArray in place.
     * Returns peak and RMS dBFS for the processed block.
     */
    @Synchronized
    fun process(
        pcm: ShortArray,
        sampleRate: Int = 48_000,
        volume: Float = 1.0f,
        gainDb: Float = 0.0f,
        pan: Float = 0.0f,
        muted: Boolean = false,
        highPassEnabled: Boolean = false,
        noiseGateThreshold: Float = 0.0f,
    ): AudioLevels {
        check(handle != 0L) { "RustAudioProcessor is closed" }
        val levels = nativeProcess(
            handle,
            pcm,
            sampleRate,
            volume,
            gainDb,
            pan,
            muted,
            highPassEnabled,
            noiseGateThreshold,
        ) ?: error("Rust could not process this PCM block")
        return AudioLevels(levels[0], levels[1])
    }

    /** Call after the audio worker stops and joins. */
    @Synchronized
    override fun close() {
        if (handle != 0L) {
            nativeDestroy(handle)
            handle = 0L
        }
    }

    private external fun nativeCreate(): Long
    private external fun nativeDestroy(handle: Long)
    private external fun nativeProcess(
        handle: Long,
        pcm: ShortArray,
        sampleRate: Int,
        volume: Float,
        gainDb: Float,
        pan: Float,
        muted: Boolean,
        highPassEnabled: Boolean,
        noiseGateThreshold: Float,
    ): FloatArray?

    companion object {
        init {
            System.loadLibrary("rust_sync_core")
        }
    }
}

data class AudioLevels(val peakDbfs: Float, val rmsDbfs: Float)
