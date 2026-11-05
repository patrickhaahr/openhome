package com.patrickhaahr.openhome.e2e

import android.os.SystemClock
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.BySelector
import androidx.test.uiautomator.StaleObjectException
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until
import java.util.regex.Pattern
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
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
private const val DOUBLE_TAP_GAP_MS = 200L
private val ANY_TEXT = Pattern.compile(".+", Pattern.DOTALL)

/** Drives the installed app from outside its process, the way the user and Android do. */
class AppDriver {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val device = UiDevice.getInstance(instrumentation)

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

    fun usesGestureNavigation(): Boolean =
        device.executeShellCommand("settings get secure navigation_mode").trim() == "2"

    /** The system back gesture: a drag inward from the screen's edge. */
    fun swipeBackGesture() {
        device.swipe(1, device.displayHeight / 2, device.displayWidth / 2, device.displayHeight / 2, 25)
    }

    fun assertLeftApp() {
        assertTrue("Back at the navigation root should leave the app", device.wait(Until.gone(By.pkg(APP_PACKAGE)), TIMEOUT_MS))
    }

    fun assertVisible(text: String): UiObject2 =
        checkNotNull(device.wait(Until.findObject(By.text(text)), TIMEOUT_MS)) { "Expected to see \"$text\"" }

    fun assertTextContaining(text: String): UiObject2 =
        checkNotNull(device.wait(Until.findObject(By.textContains(text)), TIMEOUT_MS)) {
            "Expected text containing \"$text\""
        }

    /** Synchronization runs in the background, whenever Android lets its job start. */
    fun assertEventuallyVisible(text: String) {
        checkNotNull(device.wait(Until.findObject(By.text(text)), SYNC_TIMEOUT_MS)) { "Expected to see \"$text\" eventually" }
    }

    /** Waits for a line that reads as this, however long the text around it. */
    fun assertEventuallyContaining(text: String) {
        checkNotNull(device.wait(Until.findObject(By.textContains(text)), SYNC_TIMEOUT_MS)) {
            "Expected text containing \"$text\" eventually"
        }
    }

    /** The field or control with this description, scrolled into view. */
    fun described(description: String): UiObject2 {
        val selector = By.desc(description)
        val x = device.displayWidth / 2
        val high = device.displayHeight * 2 / 3
        val low = device.displayHeight / 3
        repeat(SCROLL_ATTEMPTS) { attempt ->
            device.waitForIdle()
            device.findObject(selector)?.let { return it }
            // Downward first, then back up past where it started.
            if (attempt < SCROLL_ATTEMPTS / 2) device.swipe(x, high, x, low, SWIPE_STEPS) else device.swipe(x, low, x, high, SWIPE_STEPS)
        }
        error("Nothing described as \"$description\"")
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

    fun assertChecked(description: String, expected: Boolean) {
        described(description)
        assertTrue(
            "\"$description\" should be ${if (expected) "checked" else "unchecked"}",
            device.wait(Until.hasObject(By.desc(description).checked(expected)), TIMEOUT_MS),
        )
    }

    /** Scrolls the current list until a button or line with this text can be tapped. */
    fun tapBelow(text: String) {
        val x = device.displayWidth / 2
        repeat(SCROLL_ATTEMPTS) {
            device.waitForIdle()
            if (device.hasObject(By.text(text))) {
                tap(text)
                return
            }
            device.swipe(x, device.displayHeight * 2 / 3, x, device.displayHeight / 3, SWIPE_STEPS)
        }
        error("Nothing to tap reading \"$text\"")
    }

    /** Exactly [expected] of the names in [among] are on screen now; the rest have been filtered away. */
    fun assertExercisesShown(vararg expected: String, among: List<String>) {
        device.waitForIdle()
        SystemClock.sleep(SETTLE_MS)
        val shown = among.filter { device.hasObject(By.text(it)) }
        assertEquals(expected.toList(), shown)
    }

    /** Moves the logging focus; the Exercise's rows are then what the screen shows. */
    fun goToExercise(name: String) {
        tapScrolledTo("Go to $name")
    }

    /** Exactly the Exercises the Active Workout holds, in the order the screen offers them. */
    fun assertExerciseOrder(vararg expected: String) {
        described("Go to ${expected.first()}")
        assertEquals(expected.toList(), exerciseOrderOnScreen())
    }

    /** The screen can re-lay out under a read, so a read interrupted by that starts over from a fresh find. */
    private fun exerciseOrderOnScreen(): List<String> {
        repeat(TAP_ATTEMPTS) {
            device.waitForIdle()
            try {
                return device.findObjects(By.descStartsWith("Go to "))
                    .map { Triple(it.visibleBounds.top, it.visibleBounds.left, it.contentDescription) }
                    .sortedWith(compareBy({ it.first }, { it.second }))
                    .map { it.third.removePrefix("Go to ") }
            } catch (_: StaleObjectException) {
                // Find them all again.
            }
        }
        error("The Exercises kept changing and could not be read")
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

    fun assertNotDescribed(description: String) {
        device.waitForIdle()
        assertNull("Did not expect \"$description\"", device.wait(Until.findObject(By.desc(description)), ABSENCE_WAIT_MS))
    }

    fun assertTextAbsent(fragment: String) {
        device.waitForIdle()
        assertNull("Did not expect text containing \"$fragment\"", device.wait(Until.findObject(By.textContains(fragment)), ABSENCE_WAIT_MS))
    }

    fun assertAbsent(text: String) {
        device.waitForIdle()
        assertNull("Did not expect to see \"$text\"", device.wait(Until.findObject(By.text(text)), ABSENCE_WAIT_MS))
    }

    fun tap(text: String) {
        tapWhenSettled(By.text(text))
    }

    fun tapTextContaining(text: String) {
        tapWhenSettled(By.textContains(text))
    }

    /** For asserting that something does not happen, given time it would need if it did. */
    fun sleepFor(millis: Long) {
        SystemClock.sleep(millis)
    }

    fun tapDescription(description: String) {
        tapWhenSettled(By.desc(description))
    }

    /** The app ignores taps until a destination's transition ends; recomposition can replace the node. */
    private fun tapWhenSettled(selector: BySelector) {
        repeat(TAP_ATTEMPTS) {
            device.waitForIdle()
            SystemClock.sleep(SETTLE_MS)
            val target =
                checkNotNull(device.wait(Until.findObject(selector), TIMEOUT_MS)) { "Expected to see $selector" }
            try {
                target.click()
                return
            } catch (_: StaleObjectException) {
                // Find it again.
            }
        }
        error("$selector kept changing and could not be tapped")
    }

    /**
     * Flings the screen's list downward until [text] appears; lists can be hundreds of rows long.
     * A busy device can drop one quick swipe, so only repeated stillness means the list has ended.
     */
    fun scrollDownUntilVisible(text: String) {
        val x = device.displayWidth / 2
        var visible: List<String>? = null
        var stillPasses = 0
        while (!device.hasObject(By.text(text))) {
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

    /** Null while the list is still moving and its nodes are being replaced. */
    private fun visibleTexts(): List<String>? =
        try {
            device.findObjects(By.pkg(APP_PACKAGE).text(ANY_TEXT)).map { it.text }
        } catch (_: StaleObjectException) {
            null
        }

    fun enter(fieldLabel: String, value: String) {
        val field =
            checkNotNull(
                device.wait(
                    Until.findObject(By.clazz("android.widget.EditText").hasDescendant(By.text(fieldLabel))),
                    TIMEOUT_MS,
                ),
            ) { "No field labelled \"$fieldLabel\"" }
        field.text = value
    }

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

    /** The labels along the bottom of the screen, in order, and which of them is the selected tab. */
    fun assertBottomTabs(vararg expected: String, selected: String) {
        device.waitForIdle()
        val labels =
            device.findObjects(By.pkg(APP_PACKAGE).text(ANY_TEXT))
                .filter { it.visibleBounds.top > device.displayHeight * 8 / 10 }
                .sortedBy { it.visibleBounds.left }
                .map { it.text }
        assertEquals(expected.toList(), labels)
        assertEquals(selected, device.findObject(By.selected(true)).findObject(By.text(ANY_TEXT)).text)
    }

    fun assertNoBottomTabs() {
        device.waitForIdle()
        assertFalse("A tab bar always has a selected tab", device.hasObject(By.pkg(APP_PACKAGE).selected(true)))
    }

    private fun hasObject(text: String) = device.hasObject(By.text(text))
}
