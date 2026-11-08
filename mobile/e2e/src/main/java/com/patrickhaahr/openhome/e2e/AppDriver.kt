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

private const val APP_PACKAGE = "com.patrickhaahr.openhome.debug"
private const val TIMEOUT_MS = 15_000L
private const val ABSENCE_WAIT_MS = 1_500L
private const val TAP_ATTEMPTS = 5
private const val SETTLE_MS = 400L
private const val FLING_STEPS = 3
private const val END_OF_LIST_PASSES = 3
private const val SYNC_TIMEOUT_MS = 90_000L
private const val SCROLL_ATTEMPTS = 8
private const val NETWORK_TIMEOUT_MS = 45_000L
private const val POLL_MS = 500L
private const val SWIPE_STEPS = 20
private const val SWIPE_MARGIN_DP = 8
private const val DOUBLE_TAP_GAP_MS = 200L
private const val MILLIS_PER_SECOND = 1_000L
private const val SECONDS_PER_MINUTE = 60L
private const val VIBRATION_RECORD_TIMEOUT_MS = 10_000L
private const val VIBRATION_SETTLE_MS = 2_000L
private val ANY_TEXT = Pattern.compile(".+", Pattern.DOTALL)
private val REST_COUNTDOWN = Pattern.compile("\\d+:\\d\\d")

/** Drives the installed app from outside its process, the way the user and Android do. */
class AppDriver {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val device = UiDevice.getInstance(instrumentation)
    private val restCountdown = By.pkg(APP_PACKAGE).text(REST_COUNTDOWN)
    private var automaticTimeSetting: String? = null
    private var clockMovedMillis = 0L
    private var vibrationsBefore = emptySet<String>()

    fun clearAppData() {
        device.executeShellCommand("pm clear $APP_PACKAGE")
    }

    /** A cold start: any running process is gone first, as after the system or the user killed it. */
    fun relaunch() {
        device.executeShellCommand("am force-stop $APP_PACKAGE")
        device.executeShellCommand("am start -W -n $APP_PACKAGE/com.patrickhaahr.openhome.MainActivity")
        assertTrue("App did not reach the foreground", device.wait(Until.hasObject(By.pkg(APP_PACKAGE)), TIMEOUT_MS))
    }

    /** The real radio: Wi-Fi off leaves the emulator's metered mobile data as the default network. */
    fun setWifi(enabled: Boolean) {
        device.executeShellCommand("svc wifi ${if (enabled) "enable" else "disable"}")
        val wanted = if (enabled) "WIFI" else "CELLULAR"
        val deadline = SystemClock.uptimeMillis() + NETWORK_TIMEOUT_MS
        while (defaultNetworkTransport() != wanted) {
            check(SystemClock.uptimeMillis() < deadline) { "Default network never became $wanted" }
            SystemClock.sleep(POLL_MS)
        }
    }

    /** The default network's transport as Android itself reports it. */
    private fun defaultNetworkTransport(): String? {
        val dump = device.executeShellCommand("dumpsys connectivity")
        val network = Regex("Active default network: (\\d+)").find(dump)?.groupValues?.get(1) ?: return null
        val agent = dump.lineSequence().firstOrNull { it.contains("NetworkAgentInfo{network{$network}") } ?: return null
        return Regex("Transports: (\\w+)").find(agent)?.groupValues?.get(1)
    }

    fun pressBackButton() {
        device.pressBack()
    }

    /** Leaves the app and ends its process, as Android does to a background app, and keeps its alarms. */
    fun killInBackground() {
        device.pressHome()
        device.waitForIdle()
        device.executeShellCommand("am kill $APP_PACKAGE")
        val deadline = SystemClock.uptimeMillis() + TIMEOUT_MS
        while (device.executeShellCommand("pidof $APP_PACKAGE").isNotBlank()) {
            check(SystemClock.uptimeMillis() < deadline) { "The app's process never ended" }
            SystemClock.sleep(POLL_MS)
        }
    }

    /** Opening the app with no force-stop first, so whatever Android was keeping for it is still there. */
    fun reopen() {
        device.executeShellCommand("am start -W -n $APP_PACKAGE/com.patrickhaahr.openhome.MainActivity")
        assertTrue("App did not reach the foreground", device.wait(Until.hasObject(By.pkg(APP_PACKAGE)), TIMEOUT_MS))
    }

    /** A force-stop, which also drops the app's alarms, leaving the app closed. */
    fun forceStop() {
        device.executeShellCommand("am force-stop $APP_PACKAGE")
    }

    /** Moves the device's real clock, the one the stored deadline and Android's alarms both go by. */
    fun moveClockForward(seconds: Long) {
        if (automaticTimeSetting == null) {
            automaticTimeSetting = device.executeShellCommand("settings get global auto_time").trim()
            device.executeShellCommand("settings put global auto_time 0")
        }
        val millis = seconds * MILLIS_PER_SECOND
        device.executeShellCommand("cmd alarm set-time ${System.currentTimeMillis() + millis}")
        clockMovedMillis += millis
    }

    /** Gives the device back the time it would have had; time that really passed still counts. */
    fun restoreClock() {
        val setting = automaticTimeSetting ?: return
        device.executeShellCommand("cmd alarm set-time ${System.currentTimeMillis() - clockMovedMillis}")
        device.executeShellCommand("settings put global auto_time $setting")
        clockMovedMillis = 0
        automaticTimeSetting = null
    }

    /** From here on, [assertVibrationsPlayed] counts. Android's record outlives tests and clearing the app. */
    fun startCountingVibrations() {
        vibrationsBefore = vibrationsOnRecord()
    }

    /**
     * Exactly [expected] vibrations by Android's own record: played to the end, not refused by its
     * background rules. A vibration is recorded when it ends, so more time is given for one to appear.
     */
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

    /** The rest bar's countdown: within the app, the one line of text that is nothing but minutes and seconds. */
    private fun restRemaining(): Long? {
        repeat(TAP_ATTEMPTS) {
            try {
                return device.findObject(restCountdown)?.text?.let { shown ->
                    val (minutes, seconds) = shown.split(":")
                    minutes.toLong() * SECONDS_PER_MINUTE + seconds.toLong()
                }
            } catch (_: StaleObjectException) {
                // Find it again.
            }
        }
        error("The rest countdown kept changing and could not be read")
    }

    /** A moved clock reaches the screen within a second or so; the wide [seconds] allow for a slow device. */
    fun assertRestRemaining(seconds: LongRange) {
        val deadline = SystemClock.uptimeMillis() + TIMEOUT_MS
        var shown = restRemaining()
        while ((shown == null || shown !in seconds) && SystemClock.uptimeMillis() < deadline) {
            SystemClock.sleep(POLL_MS)
            shown = restRemaining()
        }
        assertTrue("Rest remaining should be within $seconds, was $shown", shown != null && shown in seconds)
    }

    /** No countdown and no finished rest, given the time one would need to appear. */
    fun assertNoRest() {
        device.waitForIdle()
        assertNull("Did not expect a rest countdown", device.wait(Until.findObject(restCountdown), ABSENCE_WAIT_MS))
        assertAbsent("Rest over")
    }

    /** Moves the clock to just past the deadline of the rest on screen. */
    fun moveClockPastRestDeadline() {
        moveClockForward(checkNotNull(restRemaining()) { "No rest is running" } + 1)
        assertVisible("Rest over")
    }

    fun usesGestureNavigation(): Boolean =
        device.executeShellCommand("settings get secure navigation_mode").trim() == "2"

    /** The system back gesture: a drag inward from the screen's edge. */
    fun swipeBackGesture() {
        device.swipe(1, device.displayHeight / 2, device.displayWidth / 2, device.displayHeight / 2, 25)
    }

    fun assertLeftApp() {
        assertTrue("Back at the navigation root should leave the app", device.wait(Until.gone(By.pkg(APP_PACKAGE)), TIMEOUT_MS))
    }

    /**
     * The app says most things with symbols, keeping the words as their accessibility names, so what
     * "reads as" something is its text or its description: the way a screen reader would find it.
     */
    /** A text field showing what was typed reads as the same words as the row it found, and is not what is meant. */
    private fun findReading(text: String): UiObject2? =
        runCatching { device.findObjects(By.text(text)).firstOrNull { it.className != "android.widget.EditText" } }.getOrNull()
            ?: device.findObject(By.text(text))
            ?: device.findObject(By.desc(text))

    private fun findContaining(text: String): UiObject2? =
        device.findObject(By.textContains(text)) ?: device.findObject(By.descContains(text))

    private fun awaitObject(timeoutMs: Long, find: () -> UiObject2?): UiObject2? {
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

    /** Synchronization runs in the background, whenever Android lets its job start. */
    fun assertEventuallyVisible(text: String) {
        checkNotNull(awaitObject(SYNC_TIMEOUT_MS) { findReading(text) }) { "Expected to see \"$text\" eventually" }
    }

    /** Waits for a line that reads as this, however long the text around it. */
    fun assertEventuallyContaining(text: String) {
        checkNotNull(awaitObject(SYNC_TIMEOUT_MS) { findContaining(text) }) {
            "Expected text containing \"$text\" eventually"
        }
    }

    /** The field or control with this description, scrolled into view. */
    fun described(description: String): UiObject2 = findInScroll(By.desc(description))

    /** Text controls can move above the viewport when a preceding result changes height. */
    fun tapScrolledText(text: String) {
        findInScroll(text) { findReading(text) }
        tap(text)
    }

    private fun findInScroll(selector: BySelector): UiObject2 = findInScroll(selector.toString()) { device.findObject(selector) }

    private fun findInScroll(name: String, find: () -> UiObject2?): UiObject2 {
        // In the screen margin: nothing draggable (a hue wheel, a dimmer) lives there to swallow the swipe.
        val x = (SWIPE_MARGIN_DP * instrumentation.targetContext.resources.displayMetrics.density).toInt()
        val high = device.displayHeight * 2 / 3
        val low = device.displayHeight / 3
        repeat(SCROLL_ATTEMPTS + SCROLL_ATTEMPTS / 2) { attempt ->
            device.waitForIdle()
            find()?.let { return it }
            // Downward first, then back up past where it started.
            if (attempt < SCROLL_ATTEMPTS / 2) device.swipe(x, high, x, low, SWIPE_STEPS) else device.swipe(x, low, x, high, SWIPE_STEPS)
        }
        error("Nothing matching $name; IME shown: ${keyboardShown()}; on screen: ${onScreenNow()}")
    }

    /** A text field whose description labels it, scrolled into view. */
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

    /** Compose can expose the label below the control's enabled/disabled accessibility node, or on it. */
    fun assertButtonEnabled(description: String, expected: Boolean) {
        val label = described(description)
        assertEquals("$description enabled", expected, label.isEnabled && label.parent?.isEnabled != false)
    }

    /** The described node must itself be the toggle: any other node reads as unchecked whatever the screen shows. */
    fun assertChecked(description: String, expected: Boolean) {
        described(description)
        assertTrue(
            "\"$description\" should be ${if (expected) "checked" else "unchecked"}",
            device.wait(Until.hasObject(By.desc(description).checkable(true).checked(expected)), TIMEOUT_MS),
        )
    }

    /** A Compose Switch has children, so it carries its label on a child node and its state on itself. */
    fun assertSwitchedOn(description: String, expected: Boolean) {
        described(description)
        assertTrue(
            "\"$description\" should be ${if (expected) "on" else "off"}",
            device.wait(Until.hasObject(By.checkable(true).checked(expected).hasChild(By.desc(description))), TIMEOUT_MS),
        )
    }

    /** Scrolls the current list, either way, until a button or line reading this can be tapped. */
    fun tapBelow(text: String) {
        findInScroll(text) { findReading(text) }
        tap(text)
    }

    /** Scrolls the current list until this line is on screen. */
    fun assertVisibleBelow(text: String) {
        findInScroll(text) { findReading(text) }
    }

    /**
     * Exactly [expected] of the names in [among] end up on screen; the rest have been filtered away.
     * A filter's result arrives from storage in its own time, so nothing about idleness says it has yet.
     */
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

    /** Moves the logging focus; the Exercise's rows are then what the screen shows. */
    fun goToExercise(name: String) {
        tapScrolledTo("Go to $name")
    }

    /** What the Exercise's chip says, wherever logging has left the list scrolled. */
    fun assertExerciseProgress(name: String, progress: String) {
        described("Go to $name")
        assertVisible("$name $progress")
    }

    /**
     * Exactly the Exercises the Active Workout holds, in the order the screen offers them. Adding or removing
     * an Exercise animates the chip row, so the old chips can still be there, or nodes replaced under a read,
     * for a while. It waits until two reads in a row show the expected order, which is the settled screen, and
     * otherwise fails with the last order read.
     */
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

    /** One read of the chips; null when the screen re-laid out under it, which a later read replaces. */
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

    /** Two taps as a hurried thumb gives them: long enough apart for the screen to have changed under it. */
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

    /**
     * Closes the soft keyboard if one is up, and does nothing else: Back with no keyboard would leave the
     * screen. A tap that focuses a field raises it a moment later, so it is given the absence wait to appear.
     * While it is up, a list's fixed swipes can start on the keyboard and never scroll.
     */
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

    /** Waits for a screen element to leave, so what is asserted next is the screen that replaced it. */
    fun awaitGone(text: String) {
        check(awaitUntil(TIMEOUT_MS) { findReading(text) == null }) { "Still showing \"$text\"" }
    }

    private fun awaitUntil(timeoutMs: Long, condition: () -> Boolean): Boolean {
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

    /** For asserting that something does not happen, given time it would need if it did. */
    fun sleepFor(millis: Long) {
        SystemClock.sleep(millis)
    }

    fun tapDescription(description: String) {
        tapWhenSettled(description) { device.findObject(By.desc(description)) }
    }

    /** The app ignores taps until a destination's transition ends; recomposition can replace the node. */
    private fun tapWhenSettled(name: String, find: () -> UiObject2?) {
        repeat(TAP_ATTEMPTS) {
            device.waitForIdle()
            SystemClock.sleep(SETTLE_MS)
            val target = checkNotNull(awaitObject(TIMEOUT_MS, find)) { "Expected to see $name" }
            try {
                target.click()
                return
            } catch (_: StaleObjectException) {
                // Find it again.
            }
        }
        error("$name kept changing and could not be tapped")
    }

    /**
     * Flings the screen's list downward until [text] appears; lists can be hundreds of rows long.
     * A busy device can drop one quick swipe, so only repeated stillness means the list has ended.
     */
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
        // A tap during a fling only stops it, so let the list come to rest.
        while (true) {
            val before = visibleTexts()
            SystemClock.sleep(SETTLE_MS)
            if (before != null && before == visibleTexts()) return
        }
    }

    /** Every description and text the app shows right now, for a failure to explain itself. */
    private fun onScreenNow(): String =
        runCatching {
            device.findObjects(By.pkg(APP_PACKAGE)).mapNotNull { it.contentDescription?.takeIf(String::isNotEmpty) ?: it.text?.takeIf(String::isNotEmpty) }
        }.getOrElse { listOf("unreadable: $it") }.toString()

    /** Null while the list is still moving and its nodes are being replaced. */
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

    /** Completes the Setup Flow against [api] and waits for the app's launch destination. */
    fun completeSetup(api: FakeAxum) {
        submitSetup(api.baseUrl, API_KEY)
        assertOnWorkoutTab()
    }

    fun submitSetup(baseUrl: String, apiKey: String) {
        enter("Base URL", baseUrl)
        enter("API Key", apiKey)
        tap(if (hasObject("Try again")) "Try again" else "Connect and download")
    }

    fun assertOnSetup() {
        assertVisible("Connect to OpenHome")
    }

    fun assertOnWorkoutTab() {
        assertVisible("Last Workout")
    }

    /** The tabs along the bottom of the screen, in order, and which of them is the selected tab. */
    fun assertBottomTabs(vararg expected: String, selected: String) {
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

    /**
     * The tab bar is removed as soon as navigation settles on the destination, but the screen being left stays
     * on the accessibility tree through its exit transition. Waits until nothing selected remains, so a bar
     * that is still being removed is not mistaken for one that stayed; a bar that stays fails after the timeout.
     */
    fun assertNoBottomTabs() {
        device.waitForIdle()
        assertTrue(
            "A tab bar always has a selected tab",
            awaitUntil(TIMEOUT_MS) { !device.hasObject(By.pkg(APP_PACKAGE).selected(true)) },
        )
    }

    private fun hasObject(text: String) = findReading(text) != null
}
