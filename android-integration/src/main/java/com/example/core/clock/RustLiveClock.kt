package com.example.core.clock

/**
 * Kotlin-side lifecycle adapter for RustSyncEngine.
 *
 * Track each output independently (for example "stream" and "recording").
 * The native timeline starts on the first active output and stops only after
 * the final output stops, preventing a recording start from resetting a live
 * stream's timestamps.
 *
 * This is a migration reference, not yet wired into Roadsearch/OpenLive.
 */
class RustLiveClock {
    private val engine = RustSyncEngine()
    private val activeOutputs = mutableSetOf<String>()

    @Volatile private var lastVideoPtsUs = 0L
    @Volatile private var lastAudioPtsUs = 0L
    @Volatile private var videoFrameCount = 0L
    @Volatile private var audioSampleCount = 0L

    @Synchronized
    fun startOutput(outputId: String) {
        require(outputId.isNotBlank()) { "outputId must not be blank" }
        val wasEmpty = activeOutputs.isEmpty()
        activeOutputs.add(outputId)
        if (wasEmpty && activeOutputs.isNotEmpty()) {
            lastVideoPtsUs = 0L
            lastAudioPtsUs = 0L
            videoFrameCount = 0L
            audioSampleCount = 0L
            engine.nativeStartSession()
        }
    }

    @Synchronized
    fun stopOutput(outputId: String) {
        activeOutputs.remove(outputId)
        if (activeOutputs.isEmpty()) {
            engine.nativeStopSession()
        }
    }

    fun currentPtsNs(): Long = engine.nativeNowNs()

    fun currentPtsUs(): Long = engine.nativeNsToUs(currentPtsNs())

    fun onVideoFramePresented(ptsUs: Long) {
        lastVideoPtsUs = ptsUs
        videoFrameCount++
    }

    fun onAudioFramePresented(ptsUs: Long, samplesCount: Int) {
        lastAudioPtsUs = ptsUs
        audioSampleCount += samplesCount
    }

    fun getAvDriftMs(): Long {
        if (lastVideoPtsUs == 0L || lastAudioPtsUs == 0L) return 0L
        return (lastVideoPtsUs - lastAudioPtsUs) / 1_000L
    }

    fun getVideoFrameCount(): Long = videoFrameCount
    fun getAudioSampleCount(): Long = audioSampleCount

    @Synchronized
    fun activeOutputCount(): Int = activeOutputs.size
}
