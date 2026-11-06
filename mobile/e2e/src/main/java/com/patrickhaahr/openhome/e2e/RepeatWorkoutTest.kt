package com.patrickhaahr.openhome.e2e

import java.time.LocalDate
import java.time.format.DateTimeFormatter
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

private const val PULL_UP = 1L
private const val PLANCHE_HOLD = 2L
private const val SQUAT = 3L
private const val LEG_RAISE = 4L
private const val DIP = 5L
private const val CALF_RAISE = 6L
private val SHOWN_DAY = DateTimeFormatter.ofPattern("EEE, d MMM yyyy")

/**
 * The usual session through the real app and database: repeat a past Workout, change only what
 * differs and confirm each Exercise. Only Axum is replaced, at its HTTP boundary.
 */
class RepeatWorkoutTest {
    private val app = AppDriver()
    private val squats =
        FixtureEntry(
            SQUAT,
            listOf(
                FixtureSet(setNumber = 1, reps = 5, weightKg = 100.0, rpe = 7),
                FixtureSet(setNumber = 2, reps = 5, weightKg = 102.5, rpe = 8),
                FixtureSet(setNumber = 3, reps = 4, weightKg = 102.5),
            ),
        )
    private val pullUps =
        FixtureEntry(
            PULL_UP,
            listOf(
                FixtureSet(setNumber = 1, reps = 8, rpe = 8),
                FixtureSet(setNumber = 2, reps = 6, weightKg = 12.5),
            ),
        )
    private val holds =
        FixtureEntry(
            PLANCHE_HOLD,
            listOf(
                FixtureSet(setNumber = 1, durationSeconds = 25, rpe = 6),
                FixtureSet(setNumber = 2, durationSeconds = 20, weightKg = 2.5),
            ),
        )
    private val last =
        FixtureWorkout(
            id = 50,
            date = "2026-03-02",
            name = "Pull day",
            notes = "Felt slow",
            entries = listOf(squats, pullUps, holds),
        )

    /** Past data the API allows but today's logging would not produce. */
    private val earlier =
        FixtureWorkout(
            id = 40,
            date = "2026-01-05",
            name = "Earlier session",
            entries =
                listOf(
                    FixtureEntry(PLANCHE_HOLD, listOf(FixtureSet(setNumber = 1, durationSeconds = 40, weightKg = 5.0, rpe = 9))),
                    FixtureEntry(LEG_RAISE, listOf(FixtureSet(setNumber = 1, reps = 10, durationSeconds = 30))),
                    FixtureEntry(DIP, emptyList()),
                    FixtureEntry(PULL_UP, listOf(FixtureSet(setNumber = 1, reps = 5, weightKg = 0.0))),
                ),
        )
    private val api =
        FakeAxum(
            exercises =
                mapOf(
                    PULL_UP to "Pull-Up",
                    PLANCHE_HOLD to "Planche Hold",
                    SQUAT to "Squat",
                    LEG_RAISE to "Leg Raise",
                    DIP to "Dip",
                    CALF_RAISE to "Calf Raise",
                ),
            workouts = listOf(earlier, last),
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

    private fun repeatLast() {
        app.tap("Repeat Last Workout")
        app.assertVisible("Active Workout")
    }

    private fun finish() {
        app.tapBelow("Finish Workout")
    }

    private fun complete(exercise: String) {
        app.goToExercise(exercise)
        app.tapBelow("Complete Exercise")
    }

    /** The row's values as typed, and that it is not performed yet. */
    private fun assertUnconfirmedRow(row: String, reps: String? = null, seconds: String? = null, kg: String = "", rpe: String = "") {
        reps?.let { app.assertDescribedValue("$row reps", it) }
        seconds?.let { app.assertDescribedValue("$row seconds", it) }
        app.assertDescribedValue("$row added kg", kg)
        app.assertDescribedValue("$row RPE", rpe)
        app.assertChecked("$row performed", false)
    }

    private fun assertLastWorkoutCopiedUnconfirmed() {
        app.assertExerciseOrder("Squat", "Pull-Up", "Planche Hold")
        app.goToExercise("Squat")
        assertUnconfirmedRow("Squat set 1", reps = "5", kg = "100", rpe = "7")
        assertUnconfirmedRow("Squat set 2", reps = "5", kg = "102.5", rpe = "8")
        assertUnconfirmedRow("Squat set 3", reps = "4", kg = "102.5")
        app.assertNotDescribed("Squat set 4 reps")
        app.goToExercise("Pull-Up")
        assertUnconfirmedRow("Pull-Up set 1", reps = "8", rpe = "8")
        assertUnconfirmedRow("Pull-Up set 2", reps = "6", kg = "12.5")
        app.assertNotDescribed("Pull-Up set 3 reps")
        app.goToExercise("Planche Hold")
        assertUnconfirmedRow("Planche Hold set 1", seconds = "25", rpe = "6")
        assertUnconfirmedRow("Planche Hold set 2", seconds = "20", kg = "2.5")
        app.assertNotDescribed("Planche Hold set 3 seconds")
    }

    /** The whole typical session, unchanged: the server gets the same performance on today's date. */
    @Test
    fun repeatingTheLastWorkoutAndCompletingEachExerciseRecordsTheSamePerformanceToday() {
        app.assertVisible("Start Empty")

        // A hurried double tap still starts exactly one copy.
        app.doubleTap("Repeat Last Workout")
        app.assertVisible("Active Workout")
        app.assertVisible(LocalDate.now().format(SHOWN_DAY))
        app.assertVisible("Pull day")
        assertLastWorkoutCopiedUnconfirmed()

        // Nothing here is performance until it is confirmed, across a restart too.
        app.relaunch()
        app.assertAbsent("Repeat Last Workout")
        app.assertAbsent("Start Empty")
        app.tap("Resume Workout")
        assertLastWorkoutCopiedUnconfirmed()

        // Each Complete Exercise moves on by itself.
        app.goToExercise("Squat")
        app.tapBelow("Complete Exercise")
        app.described("Pull-Up set 1 performed")
        app.tapBelow("Complete Exercise")
        app.described("Planche Hold set 1 performed")
        app.tapBelow("Complete Exercise")
        app.assertExerciseProgress("Squat", "3/3")
        app.assertExerciseProgress("Pull-Up", "2/2")
        app.assertExerciseProgress("Planche Hold", "2/2")

        // With every Set confirmed there is nothing to review.
        finish()
        app.assertAbsent("Finish with unchecked Sets?")
        app.pressBackButton()
        app.assertEventuallyVisible("Synced with OpenHome")
        val stored = api.storedWorkouts().filter { it.id != last.id && it.id != earlier.id }.single()
        assertEquals(LocalDate.now().toString(), stored.date)
        assertEquals("Pull day", stored.name)
        assertEquals("A session's notes describe that session", null, stored.notes)
        assertEquals(last.entries, stored.entries)
        assertEquals(1, api.workoutDeliveries().size)
    }

    /** Any past Workout can be repeated, and an unfinished one is never replaced by doing so. */
    @Test
    fun repeatingFromHistoryCopiesThatWorkoutAndNeverCreatesASecondActiveWorkout() {
        app.tap("History")
        app.tap("Earlier session")
        app.tap("Repeat This Workout")
        app.assertVisible("Active Workout")
        app.assertExerciseOrder("Planche Hold", "Leg Raise", "Dip", "Pull-Up")
        assertUnconfirmedRow("Planche Hold set 1", seconds = "40", kg = "5", rpe = "9")
        app.goToExercise("Leg Raise")
        assertUnconfirmedRow("Leg Raise set 1", reps = "10", seconds = "30")
        // An entry stored without Sets is still something to log; a stored zero means bodyweight.
        app.goToExercise("Dip")
        assertUnconfirmedRow("Dip set 1", reps = "")
        app.goToExercise("Pull-Up")
        assertUnconfirmedRow("Pull-Up set 1", reps = "5")

        // Back returns to where sessions start, which now only offers this one.
        app.pressBackButton()
        app.assertOnWorkoutTab()
        app.assertAbsent("Repeat Last Workout")
        app.assertAbsent("Start Empty")

        app.tap("History")
        app.tap("Pull day")
        app.assertAbsent("Repeat This Workout")
        app.assertVisible("Finish or discard your Active Workout to repeat this one.")
        app.tap("Resume Workout")
        app.assertVisible("Active Workout")
        app.assertExerciseOrder("Planche Hold", "Leg Raise", "Dip", "Pull-Up")

        complete("Planche Hold")
        complete("Leg Raise")
        finish()
        app.assertVisible("Dip: 1 unchecked Set")
        app.assertVisible("Pull-Up: 1 unchecked Set")
        app.tap("Omit and finish")
        app.pressBackButton()
        app.assertEventuallyVisible("Synced with OpenHome")
        val stored = api.storedWorkouts().filter { it.id != last.id && it.id != earlier.id }.single()
        assertEquals(earlier.entries.take(2), stored.entries)
    }

    /** Moving around changes nothing about what counts as performed, and neither does a restart. */
    @Test
    fun navigatingNeverConfirmsAndCompleteExerciseConfirmsOnlyItsOwnRows() {
        repeatLast()
        app.goToExercise("Pull-Up")
        app.goToExercise("Planche Hold")
        app.goToExercise("Squat")
        app.goToExercise("Planche Hold")
        app.assertExerciseProgress("Squat", "0/3")
        app.assertExerciseProgress("Pull-Up", "0/2")
        app.assertExerciseProgress("Planche Hold", "0/2")
        finish()
        app.assertVisible("No confirmed Sets")
        app.tap("Return to logging")

        // Today's deviation: one heavier Set, confirmed by the same explicit action.
        app.goToExercise("Squat")
        app.enterDescribed("Squat set 1 added kg", "105")
        app.tapBelow("Complete Exercise")
        app.described("Pull-Up set 1 performed")
        app.assertExerciseProgress("Squat", "3/3")
        app.assertExerciseProgress("Pull-Up", "0/2")

        // Confirmed rows stay editable, and can be taken back one at a time.
        app.goToExercise("Squat")
        app.assertChecked("Squat set 1 performed", true)
        app.tapScrolledTo("Squat set 3 performed")
        app.assertChecked("Squat set 3 performed", false)
        app.enterDescribed("Squat set 2 reps", "6")
        app.assertChecked("Squat set 2 performed", true)
        app.assertExerciseProgress("Squat", "2/3")

        // The focus, edits and confirmations are all still there after losing the process.
        app.goToExercise("Pull-Up")
        app.relaunch()
        app.tap("Resume Workout")
        app.described("Pull-Up set 1 performed")
        app.assertExerciseProgress("Squat", "2/3")
        app.goToExercise("Squat")
        app.assertDescribedValue("Squat set 1 added kg", "105")
        app.assertChecked("Squat set 1 performed", true)
        app.assertDescribedValue("Squat set 2 reps", "6")
        app.assertChecked("Squat set 3 performed", false)

        // Planned stays distinct from performed: unchecked rows are named, then left out.
        finish()
        app.assertVisible("Finish with unchecked Sets?")
        app.assertVisible("Squat: 1 unchecked Set")
        app.assertVisible("Pull-Up: 2 unchecked Sets")
        app.assertVisible("Planche Hold: 2 unchecked Sets")
        app.tap("Omit and finish")
        app.pressBackButton()
        app.assertEventuallyVisible("Synced with OpenHome")
        val stored = api.storedWorkouts().filter { it.id != last.id && it.id != earlier.id }.single()
        assertEquals(
            listOf(
                FixtureEntry(
                    SQUAT,
                    listOf(
                        FixtureSet(setNumber = 1, reps = 5, weightKg = 105.0, rpe = 7),
                        FixtureSet(setNumber = 2, reps = 6, weightKg = 102.5, rpe = 8),
                    ),
                ),
            ),
            stored.entries,
        )
    }

    @Test
    fun completeExerciseConfirmsOnlyValidRowsAndAdvancesToTheNextUnconfirmedExercise() {
        repeatLast()

        // A row that does not parse stays out, and says why on returning to it.
        app.goToExercise("Pull-Up")
        app.enterDescribed("Pull-Up set 1 reps", "")
        app.tapBelow("Complete Exercise")
        app.described("Planche Hold set 1 performed")
        app.assertExerciseProgress("Pull-Up", "1/2")
        app.goToExercise("Pull-Up")
        app.assertChecked("Pull-Up set 1 performed", false)
        app.assertChecked("Pull-Up set 2 performed", true)
        app.assertVisible("Enter reps to confirm")

        // Advancing goes past what is already confirmed, and wraps around to what is left.
        app.enterDescribed("Pull-Up set 1 reps", "8")
        app.tapBelow("Complete Exercise")
        app.described("Planche Hold set 1 performed")
        app.tapBelow("Complete Exercise")
        app.described("Squat set 1 performed")
        app.tapBelow("Complete Exercise")

        // Everything confirmed: no Exercise left to advance to, and it stays put.
        app.described("Squat set 1 performed")
        app.assertExerciseProgress("Squat", "3/3")
        app.assertExerciseProgress("Pull-Up", "2/2")
        app.assertExerciseProgress("Planche Hold", "2/2")

        // The second half of a double tap must not silently confirm the Exercise that just appeared.
        app.goToExercise("Squat")
        app.tapScrolledTo("Squat set 1 performed")
        app.goToExercise("Pull-Up")
        app.tapScrolledTo("Pull-Up set 1 performed")
        app.goToExercise("Squat")
        app.doubleTapDescription("Complete Squat")
        app.described("Pull-Up set 1 performed")
        app.assertAbsent("Return to logging")
        app.assertExerciseProgress("Squat", "3/3")
        app.assertExerciseProgress("Pull-Up", "1/2")
        app.assertChecked("Pull-Up set 1 performed", false)
    }

    @Test
    fun completeExerciseWithNothingValidStaysAndConfirmsNothing() {
        app.tap("Start Empty")
        app.tapBelow("Add Exercise")
        app.tap("Dip")
        app.described("Dip set 1 performed")
        app.tapBelow("Add Exercise")
        app.tap("Calf Raise")
        app.goToExercise("Dip")

        app.tapBelow("Complete Exercise")
        app.assertVisible("Enter reps to confirm")
        app.described("Dip set 1 performed")
        app.assertExerciseProgress("Dip", "0/1")
        app.assertChecked("Dip set 1 performed", false)
    }

    @Test
    fun loggingShowsWhatWasLastPerformedAndCanRepeatTheLastRow() {
        app.tap("Start Empty")
        app.tapBelow("Add Exercise")
        app.tap("Pull-Up")

        // The most recent Workout's actual Sets, not a target.
        app.assertVisible("Last time · ${LocalDate.parse(last.date).format(SHOWN_DAY)}")
        app.assertVisible("Set 1: 8 reps · Bodyweight · RPE 8")
        app.assertVisible("Set 2: 6 reps · +12.5 kg")

        // Repeating copies the row as typed, unconfirmed even when the row it copied is confirmed.
        app.enterDescribed("Pull-Up set 1 reps", "7")
        app.enterDescribed("Pull-Up set 1 added kg", "10")
        app.enterDescribed("Pull-Up set 1 RPE", "9")
        app.tapScrolledTo("Pull-Up set 1 performed")
        app.assertChecked("Pull-Up set 1 performed", true)
        app.tapScrolledTo("Repeat last set of Pull-Up")
        assertUnconfirmedRow("Pull-Up set 2", reps = "7", kg = "10", rpe = "9")
        app.assertChecked("Pull-Up set 1 performed", true)

        // Adding a blank row and removing an unneeded one are both a tap.
        app.tapScrolledTo("Add set to Pull-Up")
        assertUnconfirmedRow("Pull-Up set 3", reps = "")
        app.tapScrolledTo("Remove last set of Pull-Up")
        app.assertNotDescribed("Pull-Up set 3 reps")

        // An Exercise never logged has no previous performance to show.
        app.tapBelow("Add Exercise")
        app.tap("Calf Raise")
        app.described("Calf Raise set 1 performed")
        app.assertAbsent("Set 1: 8 reps · Bodyweight · RPE 8")
        app.assertTextAbsent("Last time")

        // Exercise changes while logging: the focus moves off a removed Exercise, keeping the rest.
        app.tapScrolledTo("Remove Calf Raise")
        app.assertExerciseOrder("Pull-Up")
        app.assertDescribedValue("Pull-Up set 2 reps", "7")
    }

    /** A Workout finished on the device is the last Workout, before Axum has ever heard of it. */
    @Test
    fun aWorkoutFinishedOfflineAndPendingSyncIsWhatRepeatAndPreviousPerformanceUse() {
        app.setWifi(false)
        repeatLast()
        app.enterDescribed("Squat set 1 reps", "3")
        app.tapBelow("Complete Exercise")
        app.described("Pull-Up set 1 performed")
        finish()
        app.tap("Omit and finish")
        app.assertVisible("Saved on device · Waiting for Wi-Fi")
        app.pressBackButton()
        assertEquals("Mobile data must not send anything", emptyList<String>(), api.workoutDeliveries())

        // Only what was performed in that session is repeated, and none of it counts yet.
        app.relaunch()
        repeatLast()
        app.assertExerciseOrder("Squat")
        assertUnconfirmedRow("Squat set 1", reps = "3", kg = "100", rpe = "7")
        assertUnconfirmedRow("Squat set 2", reps = "5", kg = "102.5", rpe = "8")
        app.assertVisible("Last time · ${LocalDate.now().format(SHOWN_DAY)}")
        app.assertVisible("Set 1: 3 reps · +100 kg · RPE 7")
    }

    @Test
    fun withNoHistoryOnlyStartEmptyIsOffered() {
        FakeAxum(exercises = mapOf(PULL_UP to "Pull-Up")).use { empty ->
            app.clearAppData()
            app.relaunch()
            app.completeSetup(empty)

            app.assertVisible("No Workouts yet")
            app.assertAbsent("Repeat Last Workout")
            app.tap("Start Empty")
            app.assertVisible("Active Workout")
            app.pressBackButton()
            app.assertVisible("Resume Workout")
            app.assertAbsent("Start Empty")
        }
    }
}
