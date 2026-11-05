package com.patrickhaahr.openhome.e2e

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

private const val PULL_UP = 1L
private const val PLANCHE_HOLD = 2L
private const val SQUAT = 3L
private const val PULL_DAY_ID = 40L
private const val SETTLED_MS = 4_000L
private const val STORED_POLLS = 90
private const val STORED_POLL_MS = 1_000L
private const val SYNCED = "Synced with OpenHome"
private const val WAITING_FOR_WIFI = "Saved on device · Waiting for Wi-Fi"
private const val CANT_REACH = "Saved on device · Can't reach OpenHome, will retry"
private const val REFUSED = "Saved on device · OpenHome refused this change"
private const val TAKEN_NAME = "OpenHome already has an Exercise with this name"

/**
 * Correcting finished Workouts from History through the real app, its real database, real process
 * restarts and the emulator's real radios. Only Axum is replaced, at its HTTP boundary.
 */
class WorkoutCorrectionTest {
    private val app = AppDriver()
    private val pullDay =
        FixtureWorkout(
            id = PULL_DAY_ID,
            date = "2026-03-02",
            name = "Pull day",
            notes = "Felt heavy",
            entries =
                listOf(
                    FixtureEntry(
                        PULL_UP,
                        notes = "Wide grip",
                        sets =
                            listOf(
                                FixtureSet(setNumber = 1, reps = 8, notes = "Slow"),
                                FixtureSet(setNumber = 2, reps = 6, weightKg = 10.0, rpe = 8),
                            ),
                    ),
                    FixtureEntry(PLANCHE_HOLD, listOf(FixtureSet(setNumber = 1, durationSeconds = 20))),
                ),
            // An earlier install's corrections: this one has to outrank them, even after a fresh download.
            clientRevision = 4,
        )
    private val laterSession =
        FixtureWorkout(
            id = 41,
            date = "2026-03-05",
            name = "Later session",
            entries = listOf(FixtureEntry(PULL_UP, listOf(FixtureSet(setNumber = 1, reps = 5)))),
        )
    private val api =
        FakeAxum(
            exercises = mapOf(PULL_UP to "Pull-Up", PLANCHE_HOLD to "Planche Hold", SQUAT to "Squat"),
            workouts = listOf(pullDay, laterSession),
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

    private fun openFromHistory(workoutName: String) {
        app.tapBelow("History")
        app.tap(workoutName)
    }

    private fun rename(from: String, to: String) {
        app.tap("Edit Workout")
        app.enter("Name (optional)", to)
        app.tap("Save Changes")
        app.assertVisible(to)
        app.assertTextAbsent(from)
    }

    /** One confirmed Set of [exercise] from an empty session, finished under [name], ending on History. */
    private fun logWorkout(name: String, exercise: String = "Pull-Up") {
        app.tap("Start Empty")
        app.assertVisible("Active Workout")
        app.tap("Add name or notes")
        app.enter("Name (optional)", name)
        app.tap("Done")
        app.tapBelow("Add Exercise")
        app.assertVisible("Search Exercises")
        app.tap(exercise)
        app.enterDescribed("$exercise set 1 reps", "8")
        app.tapScrolledTo("$exercise set 1 performed")
        app.assertChecked("$exercise set 1 performed", true)
        app.tapBelow("Finish Workout")
        app.assertVisible(name)
    }

    private fun stored(id: Long): FixtureWorkout = api.storedWorkouts().single { it.id == id }

    private fun storedNamed(name: String): FixtureWorkout = api.storedWorkouts().single { it.name == name }

    private fun newlyStored(): List<FixtureWorkout> =
        api.storedWorkouts().filter { it.id != PULL_DAY_ID && it.id != laterSession.id }

    /** Every kind of correction, offline, through a restart; then one delivery and the final record. */
    @Test
    fun correctionOnMobileDataSurvivesARestartThenReachesAxumOnceOnWifi() {
        app.setWifi(false)
        openFromHistory("Pull day")
        app.assertVisible("Felt heavy")
        app.tap("Edit Workout")

        // Date and optional metadata, with a blank clearing the notes rather than keeping them.
        app.tap("Mon, 2 Mar 2026")
        app.tapTextContaining("Saturday, March 7, 2026")
        app.tap("Set date")
        app.assertVisible("Sat, 7 Mar 2026")
        app.enter("Name (optional)", "Pull day corrected")
        app.enter("Notes (optional)", "")

        // Performance: values, one Set removed and one added, a Timed Hold, the order, a new Exercise.
        app.enterDescribed("Pull-Up set 1 reps", "9")
        app.enterDescribed("Pull-Up set 1 added kg", "12,5")
        app.enterDescribed("Pull-Up set 1 RPE", "8")
        app.tapScrolledTo("Remove Pull-Up set 2")
        app.tapScrolledTo("Add set to Pull-Up")
        app.enterDescribed("Pull-Up set 2 reps", "7")
        app.enterDescribed("Planche Hold set 1 seconds", "25")
        app.tapScrolledTo("Move Planche Hold up")
        app.tapBelow("Add Exercise")
        app.assertVisible("Search Exercises")
        app.tap("Squat")
        app.enterDescribed("Squat set 1 reps", "5")
        app.tap("Save Changes")

        // Saved on the device, and already what the app shows everywhere.
        app.assertVisible("Pull day corrected")
        app.assertVisible(WAITING_FOR_WIFI)
        app.assertAbsent("Felt heavy")
        app.scrollDownUntilVisible("9 reps · +12.5 kg")
        app.pressBackButton()
        app.assertVisible("History")
        app.assertVisible("2 Workouts on this device")
        app.assertVisible("Sat, 7 Mar 2026")
        app.pressBackButton()
        app.assertOnWorkoutTab()

        // The new date makes it the last Workout, which losing the process does not change.
        app.relaunch()
        app.assertVisible("Pull day corrected")
        openFromHistory("Pull day corrected")
        app.assertVisible(WAITING_FOR_WIFI)
        app.assertVisible("25 s · Bodyweight")
        app.scrollDownUntilVisible("7 reps · Bodyweight")
        app.scrollDownUntilVisible("5 reps · Bodyweight")
        app.sleepFor(SETTLED_MS)
        assertEquals("Mobile data must not send anything", emptyList<Long>(), api.editDeliveries())
        app.pressBackButton()
        app.pressBackButton()
        app.assertOnWorkoutTab()

        app.setWifi(true)

        app.assertEventuallyVisible(SYNCED)
        // The same record in its new order, including the notes this form has no field for.
        assertEquals(
            FixtureWorkout(
                id = PULL_DAY_ID,
                date = "2026-03-07",
                name = "Pull day corrected",
                notes = null,
                entries =
                    listOf(
                        FixtureEntry(PLANCHE_HOLD, listOf(FixtureSet(setNumber = 1, durationSeconds = 25))),
                        FixtureEntry(
                            PULL_UP,
                            notes = "Wide grip",
                            sets =
                                listOf(
                                    FixtureSet(setNumber = 1, reps = 9, weightKg = 12.5, rpe = 8, notes = "Slow"),
                                    FixtureSet(setNumber = 2, reps = 7),
                                ),
                        ),
                        FixtureEntry(SQUAT, listOf(FixtureSet(setNumber = 1, reps = 5))),
                    ),
                clientRevision = 5,
            ),
            stored(PULL_DAY_ID),
        )
        assertEquals(listOf(5L), api.editDeliveries())
        assertEquals("Nothing else changes", laterSession, stored(laterSession.id))
        assertEquals(emptyList<String>(), api.workoutDeliveries())
    }

    /** The newest correction wins, however many there were, and arrives after the create it depends on. */
    @Test
    fun correctionsBeforeTheCreateSyncedNeverBecomeASecondWorkout() {
        app.setWifi(false)
        logWorkout("Gym offline")
        app.tap("Gym offline")
        rename("Gym offline", "Once")
        app.relaunch()
        openFromHistory("Once")
        app.tap("Edit Workout")
        app.enter("Name (optional)", "Twice")
        app.enterDescribed("Pull-Up set 1 reps", "9")
        app.tap("Save Changes")
        app.assertVisible("9 reps · Bodyweight")

        app.setWifi(true)

        app.assertEventuallyVisible(SYNCED)
        val workout = newlyStored().single()
        assertEquals("Twice", workout.name)
        assertEquals(listOf(FixtureSet(setNumber = 1, reps = 9)), workout.entries.single().sets)
        assertEquals(listOf("workout:Gym offline", "edit:Twice"), api.commitOrder())
        assertEquals(1, api.workoutDeliveries().size)
    }

    /** The create committed without an answer, so its retry and the correction must land on that Workout. */
    @Test
    fun aCorrectionBehindACommittedCreateWithALostResponseEndsAsOneWorkout() {
        api.loseWorkoutResponses()
        logWorkout("Lossy session")
        app.assertEventuallyVisible(CANT_REACH)
        assertEquals(1, newlyStored().size)

        app.tap("Lossy session")
        rename("Lossy session", "Corrected session")
        app.assertEventuallyVisible(CANT_REACH)
        assertEquals("It has no server id to go to yet", emptyList<Long>(), api.editDeliveries())

        api.healthy()
        app.tap("Sync now")

        app.assertEventuallyVisible(SYNCED)
        assertEquals("Corrected session", newlyStored().single().name)
        assertEquals("Every delivery is the same logical Workout", 1, api.workoutDeliveries().toSet().size)
    }

    /** The edit commits, its answer is lost, and neither retries nor restarts can bring an older one back. */
    @Test
    fun aLostEditResponseRetriesItsRevisionAndANewerCorrectionStaysOnTop() {
        api.loseEditResponses()
        openFromHistory("Pull day")
        rename("Pull day", "First try")
        app.assertEventuallyVisible(CANT_REACH)
        assertEquals("First try", stored(PULL_DAY_ID).name)

        rename("First try", "Second try")
        waitUntil("The newer correction should commit") { stored(PULL_DAY_ID).name == "Second try" }

        app.relaunch()
        app.assertEventuallyVisible(CANT_REACH)
        waitUntil("A restart should retry") { api.editDeliveries().count { it == 6L } >= 2 }
        assertEquals("Second try", stored(PULL_DAY_ID).name)

        api.healthy()
        app.tap("Sync now")

        app.assertEventuallyVisible(SYNCED)
        val deliveries = api.editDeliveries()
        assertEquals(5L, deliveries.first())
        assertEquals(6L, deliveries.last())
        assertTrue("A retry never goes back to an older revision: $deliveries", deliveries == deliveries.sorted())
        assertEquals("Second try", stored(PULL_DAY_ID).name)
        assertEquals(2, api.storedWorkouts().size)
    }

    /** Axum never accepted the create, so correcting what it refused sends the corrected Workout instead. */
    @Test
    fun correctingARefusedCreateDeliversOneCorrectedWorkout() {
        api.refuseWorkoutCreates()
        logWorkout("Refused session")
        app.assertEventuallyVisible(REFUSED)

        api.healthy()
        app.tap("Refused session")
        rename("Refused session", "Accepted session")

        app.assertEventuallyVisible(SYNCED)
        assertEquals("Accepted session", newlyStored().single().name)
        assertEquals(emptyList<Long>(), api.editDeliveries())
    }

    /** It can never succeed, so it waits for the user with the correction intact and everything else moves on. */
    @Test
    fun anEditOfAWorkoutDeletedElsewhereIsRefusedWithoutHoldingUpTheQueue() {
        api.deleteWorkout(PULL_DAY_ID)
        app.setWifi(false)
        openFromHistory("Pull day")
        rename("Pull day", "Gone elsewhere")
        app.pressBackButton()
        app.pressBackButton()
        logWorkout("Queued behind it")

        app.setWifi(true)

        app.assertEventuallyVisible(REFUSED)
        waitUntil("The Workout queued behind it should arrive") { newlyStored().any { it.name == "Queued behind it" } }
        app.relaunch()
        openFromHistory("Gone elsewhere")
        app.assertVisible(REFUSED)
        app.tap("Sync now")
        app.assertEventuallyVisible(REFUSED)
        app.assertVisible("8 reps · Bodyweight")
        assertEquals(1, newlyStored().size)
    }

    /** A queued correction names local Exercises too, and has to follow when two of them turn out to be one. */
    @Test
    fun aQueuedCorrectionFollowsItsExerciseWhenItMergesIntoTheRowHoldingOpenHomesId() {
        api.putExercise(FixtureExercise(PULL_UP, "Ring Row", "calisthenics", "back"))
        app.setWifi(false)
        app.tapBelow("Library")
        app.tap("Create Exercise")
        app.enter("Name", "Ring Row")
        app.tap("Calisthenics")
        app.tap("Save Exercise")
        app.assertVisible("Ring Row")
        app.pressBackButton()
        logWorkout("Merged session", exercise = "Ring Row")
        app.tap("Merged session")
        app.tap("Edit Workout")
        app.enterDescribed("Ring Row set 1 reps", "9")
        app.tap("Save Changes")
        app.assertVisible("9 reps · Bodyweight")
        app.pressBackButton()
        app.pressBackButton()

        app.setWifi(true)
        app.assertEventuallyContaining(TAKEN_NAME)
        app.tapTextContaining(TAKEN_NAME)
        app.tap("Use OpenHome's Exercise")

        app.assertEventuallyVisible(SYNCED)
        val workout = newlyStored().single()
        assertEquals(PULL_UP, workout.entries.single().exerciseId)
        assertEquals(listOf(FixtureSet(setNumber = 1, reps = 9)), workout.entries.single().sets)
        assertEquals(listOf(1L), api.editDeliveries())
    }

    @Test
    fun onlyValidChangesSaveAndBackAsksBeforeDroppingThemThenReturnsToHistory() {
        openFromHistory("Pull day")
        val detail = "8 reps · Bodyweight"
        app.assertVisible(detail)

        // Nothing changed, so back asks nothing and saving queues nothing.
        app.tap("Edit Workout")
        app.pressBackButton()
        app.assertVisible(detail)
        app.tap("Edit Workout")
        app.tap("Save Changes")
        app.assertVisible(detail)

        // Under the same rules as logging.
        app.tap("Edit Workout")
        app.enterDescribed("Pull-Up set 1 RPE", "11")
        app.assertVisible("RPE must be 1 to 10")
        app.tap("Save Changes")
        app.assertVisible("Fix the marked Sets to save.")
        app.enterDescribed("Pull-Up set 1 RPE", "")
        app.enterDescribed("Pull-Up set 1 reps", "")
        app.tap("Save Changes")
        app.assertVisible("Enter reps to confirm")
        app.enterDescribed("Pull-Up set 1 reps", "9")

        // A Workout keeps at least one Set.
        app.tapScrolledTo("Remove Pull-Up")
        app.tapScrolledTo("Remove Planche Hold")
        app.tap("Save Changes")
        app.assertVisible("A Workout needs at least one Set.")

        app.pressBackButton()
        app.assertVisible("Discard your changes?")
        app.tap("Keep editing")
        app.assertVisible("Save Changes")
        app.pressBackButton()
        app.tap("Discard changes")

        app.assertVisible(detail)
        app.assertAbsent("Save Changes")
        app.pressBackButton()
        app.assertVisible("History")
        app.assertVisible("Pull day")

        app.sleepFor(SETTLED_MS)
        assertEquals(emptyList<Long>(), api.editDeliveries())
        assertEquals(pullDay, stored(PULL_DAY_ID))
        app.pressBackButton()
        app.assertOnWorkoutTab()
    }

    private fun waitUntil(what: String, condition: () -> Boolean) {
        repeat(STORED_POLLS) {
            if (condition()) return
            app.sleepFor(STORED_POLL_MS)
        }
        error(what)
    }
}
