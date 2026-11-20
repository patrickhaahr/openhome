package com.patrickhaahr.openhome.e2e

import android.os.SystemClock
import android.view.accessibility.AccessibilityWindowInfo
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.BySelector
import androidx.test.uiautomator.StaleObjectException
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until
import java.util.regex.Pattern
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue

internal const val APP_PACKAGE = "com.patrickhaahr.openhome.debug"
internal const val TIMEOUT_MS = 15_000L
internal const val ABSENCE_WAIT_MS = 1_500L
internal const val TAP_ATTEMPTS = 5
private const val SETTLE_MS = 400L
private const val FLING_STEPS = 3
private const val END_OF_LIST_PASSES = 3
private const val SYNC_TIMEOUT_MS = 90_000L
private const val SCROLL_ATTEMPTS = 8
internal const val POLL_MS = 500L
private const val SWIPE_STEPS = 20
private const val SWIPE_MARGIN_DP = 8
private const val DOUBLE_TAP_GAP_MS = 200L
private const val MILLIS_PER_SECOND = 1_000L
private const val VIBRATION_RECORD_TIMEOUT_MS = 10_000L
private const val VIBRATION_SETTLE_MS = 2_000L
private val ANY_TEXT = Pattern.compile(".+", Pattern.DOTALL)

class AppDriver {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    internal val device = UiDevice.getInstance(instrumentation)
    private var automaticTimeSetting: String? = null
    private var clockMovedMillis = 0L
    private var vibrationsBefore = emptySet<String>()

    private fun findReading(text: String): UiObject2? =
        runCatching { device.findObjects(By.text(text)).firstOrNull { it.className != "android.widget.EditText" } }.getOrNull()
            ?: device.findObject(By.text(text))
            ?: device.findObject(By.desc(text))

    private fun findContaining(text: String): UiObject2? =
        device.findObject(By.textContains(text)) ?: device.findObject(By.descContains(text))

    internal fun awaitObject(timeoutMs: Long, find: () -> UiObject2?): UiObject2? {
        val deadline = SystemClock.uptimeMillis() + timeoutMs
        while (true) {
            find()?.let { return it }
            if (SystemClock.uptimeMillis() >= deadline) return null
            SystemClock.sleep(POLL_MS / 2)
        }
    }

    fun assertVisible(text: String): UiObject2 =
        checkNotNull(awaitObject(TIMEOUT_MS) { findReading(text) }) { "Expected to see \"$text\"" }

    fun assertTextContaining(text: String): UiObject2 =
        checkNotNull(awaitObject(TIMEOUT_MS) { findContaining(text) }) { "Expected text containing \"$text\"" }

    fun assertEventuallyVisible(text: String) {
        checkNotNull(awaitObject(SYNC_TIMEOUT_MS) { findReading(text) }) { "Expected to see \"$text\" eventually" }
    }

    fun assertEventuallyContaining(text: String) {
        checkNotNull(awaitObject(SYNC_TIMEOUT_MS) { findContaining(text) }) {
            "Expected text containing \"$text\" eventually"
        }
    }

    fun described(description: String): UiObject2 = findInScroll(By.desc(description))

    fun tapScrolledText(text: String) {
        findInScroll(text) { findReading(text) }
        tap(text)
    }

    private fun findInScroll(selector: BySelector): UiObject2 = findInScroll(selector.toString()) { device.findObject(selector) }

    private fun findInScroll(name: String, find: () -> UiObject2?): UiObject2 {
        val x = (SWIPE_MARGIN_DP * instrumentation.targetContext.resources.displayMetrics.density).toInt()
        val high = device.displayHeight * 2 / 3
        val low = device.displayHeight / 3
        repeat(SCROLL_ATTEMPTS + SCROLL_ATTEMPTS / 2) { attempt ->
            device.waitForIdle()
            find()?.let { return it }
            if (attempt < SCROLL_ATTEMPTS / 2) device.swipe(x, high, x, low, SWIPE_STEPS) else device.swipe(x, low, x, high, SWIPE_STEPS)
        }
        error("Nothing matching $name; IME shown: ${keyboardShown()}; on screen: ${onScreenNow()}")
    }

    private fun describedField(description: String): UiObject2 {
        described(description)
        return checkNotNull(
            device.wait(
                Until.findObject(By.clazz("android.widget.EditText").hasDescendant(By.desc(description))),
                TIMEOUT_MS,
            ),
        ) { "No field described as \"$description\"" }
    }

    fun enterDescribed(description: String, value: String) {
        describedField(description).text = value
        device.waitForIdle()
    }

    fun assertDescribedValue(description: String, expected: String) {
        assertEquals(description, expected, describedField(description).text.orEmpty())
    }

    fun tapScrolledTo(description: String) {
        described(description)
        tapDescription(description)
    }

    fun assertButtonEnabled(description: String, expected: Boolean) {
        val label = described(description)
        assertEquals("$description enabled", expected, label.isEnabled && label.parent?.isEnabled != false)
    }

    fun assertChecked(description: String, expected: Boolean) {
        described(description)
        assertTrue(
            "\"$description\" should be ${if (expected) "checked" else "unchecked"}",
            device.wait(Until.hasObject(By.desc(description).checkable(true).checked(expected)), TIMEOUT_MS),
        )
    }

    fun assertSwitchedOn(description: String, expected: Boolean) {
        described(description)
        assertTrue(
            "\"$description\" should be ${if (expected) "on" else "off"}",
            device.wait(Until.hasObject(By.checkable(true).checked(expected).hasChild(By.desc(description))), TIMEOUT_MS),
        )
    }

    fun tapBelow(text: String) {
        findInScroll(text) { findReading(text) }
        tap(text)
    }

    fun assertVisibleBelow(text: String) {
        findInScroll(text) { findReading(text) }
    }

    fun assertExercisesShown(vararg expected: String, among: List<String>) {
        device.waitForIdle()
        SystemClock.sleep(SETTLE_MS)
        val wanted = expected.toList()
        val deadline = SystemClock.uptimeMillis() + TIMEOUT_MS
        var shown = among.filter { device.hasObject(By.text(it)) }
        while (shown != wanted && SystemClock.uptimeMillis() < deadline) {
            SystemClock.sleep(POLL_MS)
            shown = among.filter { device.hasObject(By.text(it)) }
        }
        assertEquals(wanted, shown)
    }

    fun goToExercise(name: String) {
        tapScrolledTo("Go to $name")
    }

    fun assertExerciseProgress(name: String, progress: String) {
        described("Go to $name")
        assertVisible("$name $progress")
    }

    fun assertExerciseOrder(vararg expected: String) {
        described("Go to ${expected.first()}")
        val wanted = expected.toList()
        var shown: List<String>? = null
        awaitUntil(TIMEOUT_MS) {
            val previous = shown
            shown = exerciseOrderOnScreen() ?: previous
            shown == wanted && previous == wanted
        }
        assertEquals(wanted, shown)
    }

    private fun exerciseOrderOnScreen(): List<String>? {
        device.waitForIdle()
        return try {
            device.findObjects(By.descStartsWith("Go to "))
                .map { Triple(it.visibleBounds.top, it.visibleBounds.left, it.contentDescription) }
                .sortedWith(compareBy({ it.first }, { it.second }))
                .map { it.third.removePrefix("Go to ") }
        } catch (_: StaleObjectException) {
            null
        }
    }

    fun doubleTapDescription(description: String) {
        doubleTap(described(description))
    }

    fun doubleTap(text: String) {
        doubleTap(assertVisible(text))
    }

    private fun doubleTap(target: UiObject2) {
        val center = target.visibleCenter
        device.waitForIdle()
        SystemClock.sleep(SETTLE_MS)
        device.click(center.x, center.y)
        SystemClock.sleep(DOUBLE_TAP_GAP_MS)
        device.click(center.x, center.y)
    }

    fun dismissKeyboardIfShown() {
        device.waitForIdle()
        val deadline = SystemClock.uptimeMillis() + ABSENCE_WAIT_MS
        while (!keyboardShown()) {
            if (SystemClock.uptimeMillis() >= deadline) return
            SystemClock.sleep(POLL_MS)
        }
        device.pressBack()
        device.waitForIdle()
    }

    private fun keyboardShown(): Boolean =
        instrumentation.uiAutomation.windows.any { it.type == AccessibilityWindowInfo.TYPE_INPUT_METHOD }

    fun assertNotDescribed(description: String) {
        device.waitForIdle()
        assertNull("Did not expect \"$description\"", device.wait(Until.findObject(By.desc(description)), ABSENCE_WAIT_MS))
    }

    fun assertTextAbsent(fragment: String) {
        device.waitForIdle()
        assertNull("Did not expect text containing \"$fragment\"", awaitObject(ABSENCE_WAIT_MS) { findContaining(fragment) })
    }

    fun awaitGone(text: String) {
        check(awaitUntil(TIMEOUT_MS) { findReading(text) == null }) { "Still showing \"$text\"" }
    }

    internal fun awaitUntil(timeoutMs: Long, condition: () -> Boolean): Boolean {
        val deadline = SystemClock.uptimeMillis() + timeoutMs
        while (!condition()) {
            if (SystemClock.uptimeMillis() >= deadline) return false
            SystemClock.sleep(POLL_MS / 2)
        }
        return true
    }

    fun assertAbsent(text: String) {
        device.waitForIdle()
        assertNull("Did not expect to see \"$text\"", awaitObject(ABSENCE_WAIT_MS) { findReading(text) })
    }

    fun tap(text: String) {
        tapWhenSettled(text) { findReading(text) }
    }

    fun tapTextContaining(text: String) {
        tapWhenSettled(text) { findContaining(text) }
    }

    fun sleepFor(millis: Long) {
        SystemClock.sleep(millis)
    }

    fun tapDescription(description: String) {
        tapWhenSettled(description) { device.findObject(By.desc(description)) }
    }

    private fun tapWhenSettled(name: String, find: () -> UiObject2?) {
        repeat(TAP_ATTEMPTS) {
            device.waitForIdle()
            SystemClock.sleep(SETTLE_MS)
            val target = checkNotNull(awaitObject(TIMEOUT_MS, find)) { "Expected to see $name" }
            try {
                target.click()
                return
            } catch (_: StaleObjectException) {
            }
        }
        error("$name kept changing and could not be tapped")
    }

    fun scrollDownUntilVisible(text: String) {
        val x = device.displayWidth / 2
        var visible: List<String>? = null
        var stillPasses = 0
        while (findReading(text) == null) {
            val current = visibleTexts()
            stillPasses = if (current != null && current == visible) stillPasses + 1 else 0
            check(stillPasses < END_OF_LIST_PASSES) { "Reached the end of the list without finding \"$text\"" }
            visible = current
            device.swipe(x, device.displayHeight * 3 / 4, x, device.displayHeight / 4, FLING_STEPS)
            device.waitForIdle()
        }
        while (true) {
            val before = visibleTexts()
            SystemClock.sleep(SETTLE_MS)
            if (before != null && before == visibleTexts()) return
        }
    }

    private fun onScreenNow(): String =
        runCatching {
            device.findObjects(By.pkg(APP_PACKAGE)).mapNotNull { it.contentDescription?.takeIf(String::isNotEmpty) ?: it.text?.takeIf(String::isNotEmpty) }
        }.getOrElse { listOf("unreadable: $it") }.toString()

    private fun visibleTexts(): List<String>? =
        try {
            device.findObjects(By.pkg(APP_PACKAGE).text(ANY_TEXT)).map { it.text }
        } catch (_: StaleObjectException) {
            null
        }

    fun enter(fieldLabel: String, value: String) {
        enterDescribed(fieldLabel, value)
    }

    fun assertField(fieldLabel: String, expected: String) {
        assertDescribedValue(fieldLabel, expected)
    }

    fun showsTextContaining(text: String): Boolean = findContaining(text) != null

    internal fun hasObject(text: String) = findReading(text) != null

    fun moveClockForward(seconds: Long) {
        if (automaticTimeSetting == null) {
            automaticTimeSetting = device.executeShellCommand("settings get global auto_time").trim()
            device.executeShellCommand("settings put global auto_time 0")
        }
        val millis = seconds * MILLIS_PER_SECOND
        device.executeShellCommand("cmd alarm set-time ${System.currentTimeMillis() + millis}")
        clockMovedMillis += millis
    }

    fun restoreClock() {
        val setting = automaticTimeSetting ?: return
        device.executeShellCommand("cmd alarm set-time ${System.currentTimeMillis() - clockMovedMillis}")
        device.executeShellCommand("settings put global auto_time $setting")
        clockMovedMillis = 0
        automaticTimeSetting = null
    }

    fun startCountingVibrations() {
        vibrationsBefore = vibrationsOnRecord()
    }

    fun assertVibrationsPlayed(expected: Int) {
        val deadline = SystemClock.uptimeMillis() + VIBRATION_RECORD_TIMEOUT_MS
        while ((vibrationsOnRecord() - vibrationsBefore).size < expected && SystemClock.uptimeMillis() < deadline) {
            SystemClock.sleep(POLL_MS)
        }
        SystemClock.sleep(VIBRATION_SETTLE_MS)
        assertEquals("Vibrations played", expected, (vibrationsOnRecord() - vibrationsBefore).size)
    }

    private fun vibrationsOnRecord(): Set<String> =
        device.executeShellCommand("dumpsys vibrator_manager")
            .substringAfter("Recent vibrations:")
            .substringBefore("Aggregated vibration history:")
            .lineSequence()
            .filter { APP_PACKAGE in it && " finished |" in it && "usage: ALARM" in it }
            .toSet()
}
