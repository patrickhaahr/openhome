package com.patrickhaahr.openhome.e2e

import android.os.SystemClock
import androidx.test.uiautomator.By
import androidx.test.uiautomator.Until
import org.junit.Assert.assertTrue

private const val NETWORK_TIMEOUT_MS = 45_000L

fun AppDriver.clearAppData() {
    device.executeShellCommand("pm clear $APP_PACKAGE")
}

/** A cold start: any running process is gone first, as after the system or the user killed it. */
fun AppDriver.relaunch() {
    device.executeShellCommand("am force-stop $APP_PACKAGE")
    device.executeShellCommand("am start -W -n $APP_PACKAGE/com.patrickhaahr.openhome.MainActivity")
    assertTrue("App did not reach the foreground", device.wait(Until.hasObject(By.pkg(APP_PACKAGE)), TIMEOUT_MS))
}

/** The real radio: Wi-Fi off leaves the emulator's metered mobile data as the default network. */
fun AppDriver.setWifi(enabled: Boolean) {
    device.executeShellCommand("svc wifi ${if (enabled) "enable" else "disable"}")
    val wanted = if (enabled) "WIFI" else "CELLULAR"
    val deadline = SystemClock.uptimeMillis() + NETWORK_TIMEOUT_MS
    while (defaultNetworkTransport() != wanted) {
        check(SystemClock.uptimeMillis() < deadline) { "Default network never became $wanted" }
        SystemClock.sleep(POLL_MS)
    }
}

/** The default network's transport as Android itself reports it. */
private fun AppDriver.defaultNetworkTransport(): String? {
    val dump = device.executeShellCommand("dumpsys connectivity")
    val network = Regex("Active default network: (\\d+)").find(dump)?.groupValues?.get(1) ?: return null
    val agent = dump.lineSequence().firstOrNull { it.contains("NetworkAgentInfo{network{$network}") } ?: return null
    return Regex("Transports: (\\w+)").find(agent)?.groupValues?.get(1)
}

fun AppDriver.pressBackButton() {
    device.pressBack()
}

/** Leaves the app and ends its process, as Android does to a background app, and keeps its alarms. */
fun AppDriver.killInBackground() {
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
fun AppDriver.reopen() {
    device.executeShellCommand("am start -W -n $APP_PACKAGE/com.patrickhaahr.openhome.MainActivity")
    assertTrue("App did not reach the foreground", device.wait(Until.hasObject(By.pkg(APP_PACKAGE)), TIMEOUT_MS))
}

/** A force-stop, which also drops the app's alarms, leaving the app closed. */
fun AppDriver.forceStop() {
    device.executeShellCommand("am force-stop $APP_PACKAGE")
}

fun AppDriver.usesGestureNavigation(): Boolean =
    device.executeShellCommand("settings get secure navigation_mode").trim() == "2"

/** The system back gesture: a drag inward from the screen's edge. */
fun AppDriver.swipeBackGesture() {
    device.swipe(1, device.displayHeight / 2, device.displayWidth / 2, device.displayHeight / 2, 25)
}

fun AppDriver.assertLeftApp() {
    assertTrue("Back at the navigation root should leave the app", device.wait(Until.gone(By.pkg(APP_PACKAGE)), TIMEOUT_MS))
}
