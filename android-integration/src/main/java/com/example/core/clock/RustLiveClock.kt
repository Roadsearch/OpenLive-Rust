package com.example.core.clock

import java.util.concurrent.atomic.AtomicLong

/**
 * Kotlin-side lifecycle adapter for RustSyncEngine.
 *
 * Track each output independently (for example "stream" and "recording").
 * The native timeline starts on the first active output and stops only after
 * the final output stops, preventing a recording start from resetting live
 * stream timestamps.
 *
 * This is a migration reference, not yet wired into Roadsearch/OpenLive.
 */
class RustLiveClock {
    private val engine = RustSyncEngine()
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
            engine.nativeStartSession()
        }
    }

    @Synchronized
    fun stopOutput(outputId: String) {
        val removed = activeOutputs.remove(outputId)
        if (removed && activeOutputs.isEmpty()) {
            engine.nativeStopSession()
        }
    }

    fun currentPtsNs(): Long = engine.nativeNowNs()

    fun currentPtsUs(): Long = engine.nativeNsToUs(currentPtsNs())

    fun onVideoFramePresented(ptsUs: Long) {
        lastVideoPtsUs.set(ptsUs.coerceAtLeast(0L))
        videoFrameCount.incrementAndGet()
    }

    fun onAudioFramePresented(ptsUs: Long, samplesCount: Int) {
        lastAudioPtsUs.set(ptsUs.coerceAtLeast(0L))
        if (samplesCount > 0) audioSampleCount.addAndGet(samplesCount.toLong())
    }

    fun getAvDriftMs(): Long {
        val videoPts = lastVideoPtsUs.get()
        val audioPts = lastAudioPtsUs.get()
        if (videoPts == 0L || audioPts == 0L) return 0L
        return (videoPts - audioPts) / 1_000L
    }

    fun getVideoFrameCount(): Long = videoFrameCount.get()
    fun getAudioSampleCount(): Long = audioSampleCount.get()

    @Synchronized
    fun activeOutputCount(): Int = activeOutputs.size
}
