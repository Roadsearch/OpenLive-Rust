package com.example.core.clock

import java.util.concurrent.atomic.AtomicLong

/**
 * Small boundary that keeps output-lifecycle behavior testable without loading JNI.
 * The production implementation delegates to [RustSyncEngine].
 */
internal interface RustClockBackend {
    fun startSession()
    fun stopSession()
    fun nowNs(): Long
    fun nsToUs(timestampNs: Long): Long
}

private class JniRustClockBackend : RustClockBackend {
    private val engine = RustSyncEngine()

    override fun startSession() = engine.nativeStartSession()
    override fun stopSession() = engine.nativeStopSession()
    override fun nowNs(): Long = engine.nativeNowNs()
    override fun nsToUs(timestampNs: Long): Long = engine.nativeNsToUs(timestampNs)
}

/**
 * Kotlin-side lifecycle adapter for RustSyncEngine.
 *
 * Track each output independently (for example "stream" and "recording").
 * The native timeline starts on the first active output and stops only after
 * the final output stops, preventing a recording start from resetting live
 * stream timestamps.
 *
 * The public no-argument constructor uses JNI. The internal constructor accepts
 * a fake backend for JVM lifecycle tests.
 *
 * This is a migration reference, not yet wired into Roadsearch/OpenLive.
 */
class RustLiveClock internal constructor(private val engine: RustClockBackend) {
    constructor() : this(JniRustClockBackend())

    private val activeOutputs = mutableSetOf<String>()

    private val lastVideoPtsUs = AtomicLong(0L)
    private val lastAudioPtsUs = AtomicLong(0L)
    private val videoFrameCount = AtomicLong(0L)
    private val audioSampleCount = AtomicLong(0L)

    @Synchronized
    fun startOutput(outputId: String) {
        require(outputId.isNotBlank()) { "outputId must not be blank" }
        val added = activeOutputs.add(outputId)
        if (added && activeOutputs.size == 1) {
            lastVideoPtsUs.set(0L)
            lastAudioPtsUs.set(0L)
            videoFrameCount.set(0L)
            audioSampleCount.set(0L)
            engine.startSession()
        }
    }

    @Synchronized
    fun stopOutput(outputId: String) {
        val removed = activeOutputs.remove(outputId)
        if (removed && activeOutputs.isEmpty()) {
            engine.stopSession()
        }
    }

    fun currentPtsNs(): Long = engine.nowNs()

    fun currentPtsUs(): Long = engine.nsToUs(currentPtsNs())

    fun onVideoFramePresented(ptsUs: Long) {
        lastVideoPtsUs.set(ptsUs.coerceAtLeast(0L))
        videoFrameCount.incrementAndGet()
    }

    /**
     * Records timestamps that have already been normalized into the same
     * session-relative timebase. The result is only a useful drift estimate
     * when audio and video callbacks use that shared timebase.
     */
    fun onAudioFramePresented(ptsUs: Long, samplesCount: Int) {
        lastAudioPtsUs.set(ptsUs.coerceAtLeast(0L))
        if (samplesCount > 0) audioSampleCount.addAndGet(samplesCount.toLong())
    }

    /** Signed video-minus-audio difference in milliseconds. */
    fun getAvDriftMs(): Long {
        val videoPts = lastVideoPtsUs.get()
        val audioPts = lastAudioPtsUs.get()
        if (videoPts == 0L || audioPts == 0L) return 0L
        // Saturation avoids Long overflow if a malformed upstream PTS is supplied.
        return videoPts.saturatingSubtract(audioPts) / 1_000L
    }

    private fun Long.saturatingSubtract(other: Long): Long {
        return when {
            other > 0L && this < Long.MIN_VALUE + other -> Long.MIN_VALUE
            other < 0L && this > Long.MAX_VALUE + other -> Long.MAX_VALUE
            else -> this - other
        }
    }

    fun getVideoFrameCount(): Long = videoFrameCount.get()
    fun getAudioSampleCount(): Long = audioSampleCount.get()

    @Synchronized
    fun activeOutputCount(): Int = activeOutputs.size
}
