package com.patrickhaahr.openhome.e2e

import kotlinx.serialization.json.Json
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

/** Authenticated HTTP requests and observable UI; no household integration or inferred device state. */
class HomeSpeakerLightsTest {
    private val app = AppDriver()
    private val api = FakeAxum()

    @Before fun setup() {
        app.setWifi(true)
        app.clearAppData()
        app.relaunch()
        app.completeSetup(api)
        app.setWifi(false)
    }

    @After fun restore() {
        app.setWifi(true)
        api.close()
    }

    @Test fun mobileDataDeliversExplicitCommandsAndAcknowledgementsNeverEstablishState() {
        api.speakerCommands = listOf("volume-down", "volume-up", "bluetooth", "optical", "test")
        app.tap("Home")
        for ((label, command) in listOf("Volume −" to "volume-down", "Volume +" to "volume-up", "Bluetooth" to "bluetooth", "Optical" to "optical")) {
            app.tapScrolledTo("Speaker $label")
            app.assertVisibleBelow("$label acknowledged by Axum. Speaker state is unobserved.")
        }
        app.assertButtonEnabled("Speaker Power", false)
        app.assertNotDescribed("Speaker test")
        for (label in listOf("On", "Off")) {
            app.tapScrolledTo("Lights $label")
            app.assertVisibleBelow("$label acknowledged by Axum. Lights state is unobserved.")

        }
        assertEquals(listOf("volume-down", "volume-up", "bluetooth", "optical").map {
            Json.parseToJsonElement("{\"command\":\"$it\"}")
        }, api.speakerRequests())
        assertEquals(listOf("/api/lights/on" to "", "/api/lights/off" to ""), api.lightRequests())
    }

    @Test fun lostResponsesAndUnavailableCapabilitiesNeverQueueOrReplay() {
        app.tap("Home")
        api.loseSpeakerResponse = true
        app.tapScrolledTo("Speaker Volume +")
        app.assertVisibleBelow("Volume + failed. Axum API unreachable. Nothing queued; Speaker outcome may be unknown.")
        api.loseLightResponse = true
        app.tapScrolledTo("Lights On")
        app.assertVisibleBelow("On failed. Axum API unreachable. Nothing queued; Lights outcome may be unknown.")
        api.loseSpeakerResponse = false
        api.loseLightResponse = false
        api.lightCommandCode = 503
        app.tapScrolledTo("Lights Off")
        app.assertVisibleBelow("Off failed. Axum API answered HTTP 503. Nothing queued; Lights outcome may be unknown.")
        api.speakerCommandCode = 503
        app.tapScrolledTo("Speaker Optical")
        app.assertVisibleBelow("Optical failed. Axum API answered HTTP 503. Nothing queued; Speaker outcome may be unknown.")
        api.tvStatusCode = 503
        app.described("Speaker Volume +")
        app.tap("Refresh Speaker controls")
        app.assertVisibleBelow("Speaker controls unavailable. Axum API answered HTTP 503. Refresh to try again.")
        app.assertButtonEnabled("Speaker Volume +", false)
        app.relaunch()
        api.tvStatusCode = 200
        api.lightCommandCode = 200
        api.speakerCommandCode = 200
        app.setWifi(true)
        app.tap("Home")
        app.sleepFor(6000)
        assertEquals(2, api.speakerRequests().size)
        assertEquals(listOf("/api/lights/on" to "", "/api/lights/off" to ""), api.lightRequests())
    }
}
