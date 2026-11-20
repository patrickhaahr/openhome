package com.patrickhaahr.openhome.e2e

import android.os.SystemClock
import androidx.test.uiautomator.By
import androidx.test.uiautomator.StaleObjectException
import androidx.test.uiautomator.Until
import java.util.regex.Pattern
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue

private const val SECONDS_PER_MINUTE = 60L
private val REST_COUNTDOWN = Pattern.compile("\\d+:\\d\\d")
private val restCountdown = By.pkg(APP_PACKAGE).text(REST_COUNTDOWN)

private fun AppDriver.restRemaining(): Long? {
    repeat(TAP_ATTEMPTS) {
        try {
            return device.findObject(restCountdown)?.text?.let { shown ->
                val (minutes, seconds) = shown.split(":")
                minutes.toLong() * SECONDS_PER_MINUTE + seconds.toLong()
            }
        } catch (_: StaleObjectException) {
        }
    }
    error("The rest countdown kept changing and could not be read")
}

fun AppDriver.assertRestRemaining(seconds: LongRange) {
    val deadline = SystemClock.uptimeMillis() + TIMEOUT_MS
    var shown = restRemaining()
    while ((shown == null || shown !in seconds) && SystemClock.uptimeMillis() < deadline) {
        SystemClock.sleep(POLL_MS)
        shown = restRemaining()
    }
    assertTrue("Rest remaining should be within $seconds, was $shown", shown != null && shown in seconds)
}

fun AppDriver.assertNoRest() {
    device.waitForIdle()
    assertNull("Did not expect a rest countdown", device.wait(Until.findObject(restCountdown), ABSENCE_WAIT_MS))
    assertAbsent("Rest over")
}

fun AppDriver.moveClockPastRestDeadline() {
    moveClockForward(checkNotNull(restRemaining()) { "No rest is running" } + 1)
    assertVisible("Rest over")
}

fun AppDriver.completeSetup(api: FakeAxum) {
    submitSetup(api.baseUrl, API_KEY)
    assertOnWorkoutTab()
}

fun AppDriver.submitSetup(baseUrl: String, apiKey: String) {
    enter("Base URL", baseUrl)
    enter("API Key", apiKey)
    tap(if (hasObject("Try again")) "Try again" else "Connect and download")
}

fun AppDriver.assertOnSetup() {
    assertVisible("Connect to OpenHome")
}

fun AppDriver.assertOnWorkoutTab() {
    assertVisible("Last Workout")
}

fun AppDriver.assertBottomTabs(vararg expected: String, selected: String) {
    device.waitForIdle()
    val tabs =
        expected.map { name ->
            checkNotNull(awaitObject(TIMEOUT_MS) { device.findObject(By.pkg(APP_PACKAGE).desc(name)) }) { "No tab named $name" }
        }
    tabs.forEach { assertTrue("Tabs belong along the bottom", it.visibleBounds.top > device.displayHeight * 8 / 10) }
    assertEquals(expected.toList(), tabs.sortedBy { it.visibleBounds.left }.map { it.contentDescription })
    assertTrue(
        "Expected $selected to become the selected tab",
        device.wait(Until.hasObject(By.selected(true).desc(selected)), TIMEOUT_MS),
    )
}

fun AppDriver.assertNoBottomTabs() {
    device.waitForIdle()
    assertTrue(
        "A tab bar always has a selected tab",
        awaitUntil(TIMEOUT_MS) { !device.hasObject(By.pkg(APP_PACKAGE).selected(true)) },
    )
}
