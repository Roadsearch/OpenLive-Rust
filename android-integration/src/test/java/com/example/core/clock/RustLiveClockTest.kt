package com.example.core.clock

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class RustLiveClockTest {
    private class FakeBackend : RustClockBackend {
        var startCalls = 0
        var stopCalls = 0
        var nowNsValue = 0L

        override fun startSession() {
            startCalls += 1
        }

        override fun stopSession() {
            stopCalls += 1
            nowNsValue = 0L
        }

        override fun nowNs(): Long = nowNsValue

        override fun nsToUs(timestampNs: Long): Long = timestampNs.coerceAtLeast(0L) / 1_000L
    }

    @Test
    fun overlappingOutputsShareOneSessionAndOnlyFinalStopEndsIt() {
        val backend = FakeBackend()
        val clock = RustLiveClock(backend)

        clock.startOutput("stream")
        backend.nowNsValue = 2_000_000L
        clock.startOutput("recording")
        clock.startOutput("stream") // Duplicate start must be ignored.

        assertEquals(1, backend.startCalls)
        assertEquals(2, clock.activeOutputCount())
        assertEquals(2_000L, clock.currentPtsUs())

        clock.stopOutput("not-an-active-output")
        clock.stopOutput("stream")
        assertEquals(0, backend.stopCalls)
        assertEquals(1, clock.activeOutputCount())

        clock.stopOutput("recording")
        assertEquals(1, backend.stopCalls)
        assertEquals(0, clock.activeOutputCount())
        assertEquals(0L, clock.currentPtsNs())
    }

    @Test
    fun audioVideoDiagnosticsCountOnlyPositiveSampleCountsAndReportSignedDrift() {
        val clock = RustLiveClock(FakeBackend())

        clock.onVideoFramePresented(250_000L)
        clock.onAudioFramePresented(230_000L, 1_024)
        assertEquals(20L, clock.getAvDriftMs())
        assertEquals(1L, clock.getVideoFrameCount())
        assertEquals(1_024L, clock.getAudioSampleCount())

        clock.onVideoFramePresented(200_000L)
        clock.onAudioFramePresented(220_000L, -64)
        assertEquals(-20L, clock.getAvDriftMs())
        assertEquals(2L, clock.getVideoFrameCount())
        assertEquals(1_024L, clock.getAudioSampleCount())

        clock.onVideoFramePresented(-1L)
        assertEquals(0L, clock.getAvDriftMs())
    }

    @Test
    fun blankOutputIdsAreRejected() {
        val backend = FakeBackend()
        val clock = RustLiveClock(backend)

        assertFailsWith<IllegalArgumentException> { clock.startOutput("") }
        assertFailsWith<IllegalArgumentException> { clock.startOutput("   ") }
        assertEquals(0, backend.startCalls)
        assertEquals(0, clock.activeOutputCount())
    }

    @Test
    fun newSessionResetsDiagnostics() {
        val backend = FakeBackend()
        val clock = RustLiveClock(backend)

        clock.startOutput("stream")
        clock.onVideoFramePresented(100_000L)
        clock.onAudioFramePresented(90_000L, 512)
        clock.stopOutput("stream")
        clock.startOutput("recording")

        assertEquals(0L, clock.getVideoFrameCount())
        assertEquals(0L, clock.getAudioSampleCount())
        assertEquals(0L, clock.getAvDriftMs())
        assertEquals(2, backend.startCalls)
        assertEquals(1, backend.stopCalls)
    }
}
