package com.patrickhaahr.openhome.e2e

import kotlinx.serialization.json.Json
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** TV controls through the installed app, authenticated HTTP boundary and real Room state. */
class HomeTvTest {
    private val app = AppDriver()
    private val api = FakeAxum(exercises = mapOf(1L to "Pull-Up"))

    @Before fun setup() {
        app.setWifi(true)
        app.clearAppData()
        app.relaunch()
        app.completeSetup(api)
    }

    @After fun restore() {
        app.setWifi(true)
        api.close()
    }

    @Test fun mobileDataCommandsUseSupportedCapabilitiesAndRemoteBackReturnsHome() {
        app.setWifi(false)
        api.tvCommands = listOf("power", "up", "ok", "back", "hdmi-1", "test")
        app.tap("Home")
        app.assertVisible("TV power: unobserved")
        app.tapDescription("TV Power")
        app.assertVisible("Power acknowledged by Axum. TV state is unobserved.")
        app.assertButtonEnabled("TV Input", false)
        app.tap("Full TV remote")
        app.assertVisible("TV remote")
        app.tapScrolledTo("TV Up")
        app.assertVisible("Up acknowledged by Axum. TV state is unobserved.")
        app.assertButtonEnabled("TV Left", false)
        app.tapScrolledTo("TV OK")
        app.tapScrolledTo("TV HDMI 1")
        app.assertNotDescribed("TV test")
        app.assertButtonEnabled("TV HDMI 2", false)
        assertEquals(
            listOf("power", "up", "ok", "hdmi-1").map { Json.parseToJsonElement("{\"command\":\"$it\"}") },
            api.tvRequests(),
        )
        app.pressBackButton()
        app.assertVisible("Full TV remote")
        app.assertVisibleBelow("Speaker")
        app.assertVisibleBelow("Lights")
        app.assertVisibleBelow("Backlight")
        app.pressBackButton()
        app.assertOnWorkoutTab()
    }

    @Test fun failuresArePromptAndNeverReplayWhileFitnessAndActiveWorkoutSurvive() {
        app.setWifi(false)
        app.tapDescription("Open Profile")
        app.tap("Edit Profile")
        app.enter("Height in cm (optional)", "182")
        app.tapBelow("Save Profile")
        app.pressBackButton()
        app.tap("Start Empty")
        app.tapBelow("Add Exercise")
        app.tap("Pull-Up")
        app.enterDescribed("Pull-Up set 1 reps", "9")
        app.pressBackButton()
        app.tap("Home")
        api.loseTvResponse = true
        app.tapDescription("TV Power")
        app.assertTextContaining("Power failed.")
        app.assertTextContaining("Nothing queued")
        assertEquals(1, api.tvRequests().size)
        api.loseTvResponse = false
        api.tvCommandCode = 503
        app.tapDescription("TV Mute")
        app.assertTextContaining("HTTP 503")
        assertEquals(2, api.tvRequests().size)
        app.tap("Full TV remote")
        app.assertVisible("TV remote")
        app.pressBackButton()
        app.assertVisible("Full TV remote")
        api.tvStatusCode = 503
        app.tap("Refresh TV controls")
        app.assertTextContaining("TV controls unavailable.")
        app.assertButtonEnabled("TV Power", false)
        app.relaunch()
        app.tap("Resume Workout")
        app.assertDescribedValue("Pull-Up set 1 reps", "9")
        app.pressBackButton()
        app.tapDescription("Open Profile")
        app.assertTextContaining("182")
        assertTrue(api.profileDeliveries().isEmpty())
        app.pressBackButton()
        api.tvStatusCode = 200
        api.tvCommandCode = 200
        app.setWifi(true)
        app.tap("Home")
        app.tap("Refresh TV controls")
        app.assertVisible("TV power: unobserved")
        app.sleepFor(6000)
        assertEquals("No failed TV command may be retried after restart or reconnection", 2, api.tvRequests().size)
        app.tap("Workout")
        app.tapDescription("Open Profile")
        app.assertEventuallyVisible("Synced with OpenHome")
        assertEquals(182.0, api.storedProfile().heightCm)
    }
}
