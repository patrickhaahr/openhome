package com.patrickhaahr.openhome.e2e

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** Observable Home UI over authenticated HTTP with independent status facts and actual Room. */
class HomeBacklightTest {
    private val app = AppDriver()
    private val api = FakeAxum(exercises = mapOf(1L to "Pull-Up"))

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

    @Test fun controlsUseCurrentEffectsAndExplicitRgbBrightnessWithoutWifi() {
        app.tap("Home")
        app.tapScrolledTo("Backlight Off")
        app.tapScrolledTo("Backlight TV Sync")
        app.scrollDownUntilVisible("Apply RGB")
        app.enter("Red", "18")
        app.enter("Green", "52")
        app.enter("Blue", "86")
        app.enter("Backlight brightness (0–100)", "42")
        app.tapBelow("Apply RGB")
        app.tapBelow("Apply brightness to intended mode")
        app.tapScrolledTo("Backlight Aurora")
        app.assertNotDescribed("Backlight Forest")
        app.tapScrolledTo("Backlight Automatic")
        api.awaitAutomaticRequest()
        assertEquals(
            listOf(
                "/api/backlight/selection" to """{"mode":"off"}""",
                "/api/backlight/selection" to """{"mode":"tv-sync"}""",
                "/api/backlight/selection" to """{"mode":"color","rgb":{"red":18,"green":52,"blue":86},"brightness":42}""",
                "/api/backlight/selection" to """{"mode":"color","rgb":{"red":20,"green":40,"blue":60},"brightness":42}""",
                "/api/backlight/selection" to """{"mode":"effect","effect":"aurora","brightness":42}""",
                "/api/backlight/automatic" to "",
            ).map { (path, body) -> path to body.takeIf { it.isNotEmpty() }?.let(Json::parseToJsonElement) },
            api.backlightRequests().map { (path, body) -> path to body.takeIf { it.isNotEmpty() }?.let(Json::parseToJsonElement) },
        )
    }

    @Test fun independentIntentAcknowledgementObservationAndRefreshRemainHonest() {
        app.tap("Home")
        app.assertVisibleBelow("Intended: RGB 20, 40, 60 at 35%")
        app.assertVisibleBelow("Last acknowledged: TV Sync at 70%")
        app.assertVisibleBelow("Server command: failed · Recovery: retrying")
        app.assertVisibleBelow("Observation: Stale / unknown")
        app.assertVisibleBelow("Last observed: Off · other · 12%")
        app.assertVisibleBelow("Readback cannot confirm exact RGB or effect.")
        app.assertVisibleBelow("Manual override: until 2026-11-07T05:00:00Z")
        api.backlightStatus = JsonObject(api.backlightStatus + mapOf(
            "control_source" to JsonPrimitive("automatic"),
            "reachability" to JsonPrimitive("unreachable"),
            "acknowledged" to kotlinx.serialization.json.JsonNull,
            "observed" to kotlinx.serialization.json.JsonNull,
            "command" to JsonPrimitive("pending"),
        ))
        app.tapBelow("Refresh Backlight status")
        app.tap("Workout")
        app.tap("Home")
        app.assertVisibleBelow("Last acknowledged: None")
        app.assertVisibleBelow("Server command: pending · Recovery: retrying")
        app.assertVisibleBelow("Backlight connection: unreachable")
        app.assertVisibleBelow("Last observed: Unknown")
        app.assertVisibleBelow("Control: Automatic")
        app.assertVisibleBelow("Server bedtime: enabled · 22:30–06:00 Europe/Copenhagen")
    }

    @Test fun lostResponseAndRefreshErrorsNeverReplayAndPreserveFitnessAcrossRestart() {
        app.tapDescription("Open Profile")
        app.tap("Edit Profile")
        app.enter("Height in cm (optional)", "183")
        app.tapBelow("Save Profile")
        app.pressBackButton()
        app.tap("Start Empty")
        app.tapBelow("Add Exercise")
        app.tap("Pull-Up")
        app.enterDescribed("Pull-Up set 1 reps", "8")
        app.pressBackButton()
        app.tap("Home")
        api.loseBacklightResponse = true
        api.backlightDelayMillis = 2000
        app.tapScrolledTo("Backlight Off")
        app.assertVisibleBelow("Sending Backlight selection…")
        app.assertVisibleBelow("Axum API unreachable. Outcome may be unknown. Nothing queued or retried by Android. Refresh Backlight status before choosing again.")
        api.loseBacklightResponse = false
        api.backlightDelayMillis = 0
        api.backlightCode = 503
        app.tap("Workout")
        app.tap("Home")
        app.tapBelow("Refresh Backlight status")
        app.assertVisibleBelow("Axum API answered HTTP 503. Outcome may be unknown. Nothing queued or retried by Android. Refresh Backlight status before choosing again.")
        app.relaunch()
        app.tap("Resume Workout")
        app.assertDescribedValue("Pull-Up set 1 reps", "8")
        app.pressBackButton()
        app.tapDescription("Open Profile")
        app.assertTextContaining("183")
        assertTrue(api.profileDeliveries().isEmpty())
        app.pressBackButton()
        api.backlightCode = 200
        app.setWifi(true)
        app.tap("Home")
        app.sleepFor(6000)
        assertEquals(listOf("/api/backlight/selection", "/api/backlight/refresh"), api.backlightRequests().map { it.first })
        app.tap("Workout")
        app.tapDescription("Open Profile")
        app.assertEventuallyVisible("Synced with OpenHome")
        assertEquals(183.0, api.storedProfile().heightCm)
    }
}
