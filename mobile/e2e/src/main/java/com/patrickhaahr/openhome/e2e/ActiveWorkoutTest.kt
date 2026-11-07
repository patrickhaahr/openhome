package com.patrickhaahr.openhome.e2e

import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

private const val PULL_UP = 1L
private const val PLANCHE_HOLD = 2L
private const val SQUAT = 3L
private const val SETTLED_MS = 4_000L
private val SHOWN_DAY = DateTimeFormatter.ofPattern("EEE, d MMM yyyy")
private val PICKER_DAY = DateTimeFormatter.ofPattern("EEEE, MMMM d, yyyy", Locale.US)

/**
 * Logging, finishing and synchronizing an Active Workout through the real app, its real database,
 * real process restarts and the emulator's real radios. Only Axum is replaced, at its HTTP boundary.
 */
class ActiveWorkoutTest {
    private val app = AppDriver()
    private val past =
        FixtureWorkout(
            id = 40,
            date = "2026-01-05",
            name = "Earlier session",
            entries = listOf(FixtureEntry(PLANCHE_HOLD, listOf(FixtureSet(setNumber = 1, durationSeconds = 15)))),
        )
    private val api =
        FakeAxum(
            exercises = mapOf(PULL_UP to "Pull-Up", PLANCHE_HOLD to "Planche Hold", SQUAT to "Squat"),
            exerciseCategories = mapOf(SQUAT to "gym"),
            workouts = listOf(past),
        )

    @Before
    fun completeSetup() {
        app.setWifi(true)
        app.clearAppData()
        app.relaunch()
        app.completeSetup(api)
    }

    @After
    fun restore() {
        app.setWifi(true)
        api.close()
    }

    private fun startWithExercise(name: String) {
        app.tap("Start Empty")
        app.assertVisible("Active Workout")
        addExercise(name)
    }

    private fun addExercise(name: String) {
        app.tapBelow("Add Exercise")
        app.assertVisible("Search Exercises")
        app.tap(name)
        app.described("$name set 1 performed")
    }

    private fun logOneConfirmedPullUp(reps: String = "8") {
        startWithExercise("Pull-Up")
        app.enterDescribed("Pull-Up set 1 reps", reps)
        app.tapScrolledTo("Pull-Up set 1 performed")
        app.assertChecked("Pull-Up set 1 performed", true)
    }

    private fun finish() {
        app.tapBelow("Finish Workout")
    }

    private fun assertStoredOnce(): FixtureWorkout {
        val added = api.storedWorkouts().filter { it.id != past.id }
        assertEquals("Axum should hold exactly one new Workout", 1, added.size)
        return added.single()
    }

    /** Offline logging on mobile data, restarts, local finish and History, then Wi-Fi recovery. */
    @Test
    fun workoutLoggedAndFinishedOnMobileDataStaysQueuedThenSyncsOnceOnWifi() {
        app.setWifi(false)

        app.tap("Start Empty")
        app.assertVisible("Saved on device")
        val today = LocalDate.now()
        val backdated = today.minusDays(1)
        app.tap(today.format(SHOWN_DAY))
        if (backdated.month != today.month) app.tapDescription("Change to previous month")
        app.tapTextContaining(backdated.format(PICKER_DAY))
        app.tap("Set date")
        app.assertVisible(backdated.format(SHOWN_DAY))
        app.tap("Add name or notes")
        app.enter("Name (optional)", "Gym offline")
        app.enter("Notes (optional)", "No signal in the basement")
        app.tap("Done")

        // A mixed-category session, in the order chosen.
        addExercise("Pull-Up")
        addExercise("Planche Hold")
        addExercise("Squat")

        // Reps with Added Weight and RPE, and bodyweight with no RPE.
        app.goToExercise("Pull-Up")
        app.enterDescribed("Pull-Up set 1 reps", "8")
        app.enterDescribed("Pull-Up set 1 added kg", "12,5")
        app.enterDescribed("Pull-Up set 1 RPE", "8")
        app.tapScrolledTo("Pull-Up set 1 performed")
        app.assertChecked("Pull-Up set 1 performed", true)
        app.tapScrolledTo("Add set to Pull-Up")
        app.enterDescribed("Pull-Up set 2 reps", "6")
        app.tapScrolledTo("Pull-Up set 2 performed")
        app.assertChecked("Pull-Up set 2 performed", true)
        // Unchecking takes the Set back out, and a row left unchecked keeps its values but is not performed.
        app.tapScrolledTo("Pull-Up set 2 performed")
        app.assertChecked("Pull-Up set 2 performed", false)

        // Planche Hold was last logged as a Timed Hold, so it opens on seconds without any reps field.
        app.goToExercise("Planche Hold")
        app.enterDescribed("Planche Hold set 1 seconds", "20")
        app.tapScrolledTo("Planche Hold set 1 performed")
        app.assertChecked("Planche Hold set 1 performed", true)

        // Squat has no past Sets to go by, and the user can choose either measure.
        app.goToExercise("Squat")
        app.enterDescribed("Squat set 1 reps", "5")
        app.tapScrolledTo("Log Squat as Hold")
        app.assertDescribedValue("Squat set 1 seconds", "")
        app.tapScrolledTo("Log Squat as Reps")
        app.assertDescribedValue("Squat set 1 reps", "5")

        // Leaving by tab and by back keeps everything.
        app.tap("Home")
        app.assertVisible("Full TV remote")
        app.tap("Workout")
        app.goToExercise("Pull-Up")
        app.assertDescribedValue("Pull-Up set 1 reps", "8")
        app.pressBackButton()
        app.assertOnWorkoutTab()
        app.assertAbsent("Start Empty")

        // As does losing the process, including the half-finished rows and their confirmations.
        app.relaunch()
        app.tap("Resume Workout")
        app.assertVisible(backdated.format(SHOWN_DAY))
        app.assertVisible("Gym offline")
        app.assertDescribedValue("Pull-Up set 1 added kg", "12,5")
        app.assertChecked("Pull-Up set 1 performed", true)
        app.assertDescribedValue("Pull-Up set 2 reps", "6")
        app.assertChecked("Pull-Up set 2 performed", false)
        app.goToExercise("Planche Hold")
        app.assertChecked("Planche Hold set 1 performed", true)
        app.goToExercise("Squat")
        app.assertChecked("Squat set 1 performed", false)

        // Finishing names what is left out; returning records nothing.
        finish()
        app.assertVisible("Finish with unchecked Sets?")
        app.assertVisible("Pull-Up: 1 unchecked Set")
        app.assertVisible("Squat: 1 unchecked Set")
        app.tap("Return to logging")
        app.goToExercise("Pull-Up")
        app.assertChecked("Pull-Up set 2 performed", false)
        finish()
        app.tap("Omit and finish")

        // A local Workout in History, with no network involved.
        app.assertVisible("Gym offline")
        app.assertVisible("Saved on device · Waiting for Wi-Fi")
        app.assertTextContaining("Pull-Up · Planche Hold · 2 Sets")
        assertEquals("Mobile data must not send anything", emptyList<String>(), api.workoutDeliveries())

        // Queued work survives a restart, and only confirmed Sets became performance.
        app.relaunch()
        app.tap("History")
        app.assertVisible("Saved on device · Waiting for Wi-Fi")
        app.tap("Gym offline")
        app.assertVisible("Saved on device · Waiting for Wi-Fi")
        app.assertVisible("No signal in the basement")
        app.assertVisible("8 reps · +12.5 kg")
        app.assertVisible("20 s · Bodyweight")
        app.assertAbsent("6 reps · Bodyweight")
        app.assertAbsent("Squat")
        app.sleepFor(SETTLED_MS)
        assertEquals("Mobile data must not send anything", emptyList<String>(), api.workoutDeliveries())

        app.setWifi(true)

        app.assertEventuallyVisible("Synced with OpenHome")
        val stored = assertStoredOnce()
        assertEquals(backdated.toString(), stored.date)
        assertEquals("Gym offline", stored.name)
        assertEquals("No signal in the basement", stored.notes)
        assertEquals(
            listOf(
                FixtureEntry(PULL_UP, listOf(FixtureSet(setNumber = 1, reps = 8, weightKg = 12.5, rpe = 8))),
                FixtureEntry(PLANCHE_HOLD, listOf(FixtureSet(setNumber = 1, durationSeconds = 20))),
            ),
            stored.entries,
        )
        assertEquals(1, api.workoutDeliveries().size)

        // Same local Workout, still one row, after another restart.
        app.relaunch()
        app.assertVisible("2 Workouts on this device")
        app.assertVisible("Synced with OpenHome")
    }

    /** The server commits, the response never arrives, and both the client and the retry restart. */
    @Test
    fun committedCreateWithALostResponseRetriesAsTheSameWorkoutAcrossAClientRestart() {
        api.loseWorkoutResponses()
        logOneConfirmedPullUp()

        // An Active Workout stays on the device, even with Wi-Fi and Axum available.
        app.sleepFor(SETTLED_MS)
        assertEquals(emptyList<String>(), api.workoutDeliveries())

        finish()
        app.assertEventuallyVisible("Saved on device · Can't reach OpenHome, will retry")
        assertStoredOnce()

        app.relaunch()
        app.assertEventuallyVisible("Saved on device · Can't reach OpenHome, will retry")
        assertStoredOnce()
        assertTrue("The restart should have retried", api.workoutDeliveries().size >= 2)

        api.healthy()
        app.tap("Sync now")

        app.assertEventuallyVisible("Synced with OpenHome")
        assertStoredOnce()
        assertEquals("Every delivery is the same logical Workout", 1, api.workoutDeliveries().toSet().size)
        app.assertVisible("2 Workouts on this device")
    }

    @Test
    fun axumUnreachableOnWifiKeepsTheWorkoutQueuedAcrossRestartThenRecovers() {
        api.dropWorkoutRequests()
        logOneConfirmedPullUp()
        finish()

        app.assertEventuallyVisible("Saved on device · Can't reach OpenHome, will retry")
        app.relaunch()
        app.tap("History")
        app.assertEventuallyVisible("Saved on device · Can't reach OpenHome, will retry")
        assertEquals("Nothing was committed", listOf(past), api.storedWorkouts())

        api.failWorkoutCreates()
        app.pressBackButton()
        app.tap("Sync now")
        app.assertEventuallyVisible("Saved on device · OpenHome had a problem, will retry")

        api.healthy()
        app.tap("Sync now")
        app.assertEventuallyVisible("Synced with OpenHome")
        assertStoredOnce()
    }

    @Test
    fun refusedWorkoutStaysOnDeviceWithAnActionableStatusUntilRetried() {
        api.refuseWorkoutCreates()
        logOneConfirmedPullUp()
        finish()

        app.assertEventuallyVisible("Saved on device · OpenHome refused this change")
        app.relaunch()
        app.assertVisible("Saved on device · OpenHome refused this change")
        app.assertVisible("2 Workouts on this device")
        val attempts = api.workoutDeliveries().size
        app.sleepFor(SETTLED_MS)
        assertEquals("A refused change waits for the user", attempts, api.workoutDeliveries().size)

        api.healthy()
        app.tap("Sync now")
        app.assertEventuallyVisible("Synced with OpenHome")
        assertStoredOnce()
    }

    @Test
    fun onlyAValidSetCanBeConfirmedAndNothingUnconfirmedIsEverRecorded() {
        startWithExercise("Pull-Up")

        app.tapScrolledTo("Pull-Up set 1 performed")
        app.assertVisible("Enter reps to confirm")
        app.assertChecked("Pull-Up set 1 performed", false)

        app.enterDescribed("Pull-Up set 1 reps", "8")
        app.enterDescribed("Pull-Up set 1 RPE", "11")
        app.assertVisible("RPE must be 1 to 10")
        app.tapScrolledTo("Pull-Up set 1 performed")
        app.assertChecked("Pull-Up set 1 performed", false)

        // With nothing confirmed there is no Workout to finish.
        finish()
        app.assertVisible("No confirmed Sets")
        app.assertAbsent("Omit and finish")
        app.tap("Return to logging")

        app.enterDescribed("Pull-Up set 1 RPE", "9")
        app.tapScrolledTo("Pull-Up set 1 performed")
        app.assertChecked("Pull-Up set 1 performed", true)

        // Editing a confirmed Set into something invalid takes its confirmation away.
        app.enterDescribed("Pull-Up set 1 reps", "")
        app.assertChecked("Pull-Up set 1 performed", false)

        // Discarding is explicit, and the app is then ready to start again with nothing recorded.
        app.tapBelow("Discard Workout")
        app.assertVisible("Discard this Workout?")
        app.tap("Keep logging")
        app.assertVisible("Active Workout")
        app.tapBelow("Discard Workout")
        app.tap("Discard")
        app.assertVisible("Start Empty")
        app.assertVisible("1 Workout on this device")
        assertEquals(listOf(past), api.storedWorkouts())
    }
}
