package com.patrickhaahr.openhome.e2e

import org.junit.After
import org.junit.Before
import org.junit.Test

private const val SQUAT = 1L
private const val PULL_UP = 2L
private const val RING_ROW = "Ring Row"

/** What a fresh default rest can read after the taps that started it, however slow the device. */
private val FRESH_DEFAULT_REST = 120L..150L
private val A_MINUTE_INTO_DEFAULT_REST = 60L..90L
private const val A_MINUTE = 60L
private const val LONG_PAST_ANY_REST = 1_000L

/**
 * Rest timing through the real app, its real database, Android's real alarms and the device's real
 * clock, which the test moves instead of waiting. Only Axum is replaced, at its HTTP boundary.
 */
class RestTimerTest {
    private val app = AppDriver()
    private val api =
        FakeAxum(
            exercises = mapOf(SQUAT to "Squat", PULL_UP to "Pull-Up"),
            workouts =
                listOf(
                    FixtureWorkout(
                        id = 50,
                        date = "2026-03-02",
                        name = "Usual session",
                        entries =
                            listOf(
                                FixtureEntry(
                                    SQUAT,
                                    listOf(
                                        FixtureSet(setNumber = 1, reps = 5, weightKg = 100.0),
                                        FixtureSet(setNumber = 2, reps = 5, weightKg = 100.0),
                                        FixtureSet(setNumber = 3, reps = 5, weightKg = 100.0),
                                    ),
                                ),
                                FixtureEntry(PULL_UP, listOf(FixtureSet(setNumber = 1, reps = 8))),
                            ),
                    ),
                ),
        )

    @Before
    fun completeSetup() {
        app.setWifi(true)
        app.clearAppData()
        app.relaunch()
        app.completeSetup(api)
        app.startCountingVibrations()
    }

    @After
    fun restore() {
        app.restoreClock()
        app.setWifi(true)
        api.close()
    }

    private fun repeatLast() {
        app.tap("Repeat Last Workout")
        app.assertVisible("Active Workout")
    }

    private fun resume() {
        app.tap("Resume Workout")
        app.assertVisible("Active Workout")
    }

    private fun confirm(row: String) {
        app.tapScrolledTo("$row performed")
        app.assertChecked("$row performed", true)
    }

    /** What the Exercise's card says about its rests, which can be below the fold. */
    private fun assertCardSays(exercise: String, line: String) {
        app.described("Rest timer for $exercise")
        app.assertVisibleBelow(line)
    }

    /** Several rows, one rest, one vibration: through other tabs, a restart and a repeated press. */
    @Test
    fun completeExerciseRunsOneDefaultRestThatVibratesOnceAtItsDeadline() {
        repeatLast()
        assertCardSays("Squat", "Rest 2:30")
        app.tapBelow("Complete Exercise")

        app.assertVisible("Rest · Squat")
        app.assertRestRemaining(FRESH_DEFAULT_REST)
        app.moveClockForward(A_MINUTE)
        app.assertRestRemaining(A_MINUTE_INTO_DEFAULT_REST)

        // It follows the user around and leaves everything under it usable.
        app.tap("Home")
        app.assertVisible("Rest · Squat")
        app.assertRestRemaining(A_MINUTE_INTO_DEFAULT_REST)
        app.tapDescription("Open Profile")
        app.assertVisible("Rest · Squat")
        app.pressBackButton()
        app.tap("Workout")
        app.assertVibrationsPlayed(0)

        app.moveClockPastRestDeadline()
        app.assertVibrationsPlayed(1)

        // Showing it again is not expiring again, nor starting again.
        app.tap("Home")
        app.tap("Workout")
        app.relaunch()
        app.assertVisible("Rest over")
        resume()
        app.assertVisible("Rest over")
        app.assertVibrationsPlayed(1)

        // A second press on an Exercise with everything confirmed confirms nothing.
        app.goToExercise("Squat")
        app.tapBelow("Complete Exercise")
        app.assertVisible("Rest over")

        app.tapDescription("Dismiss rest")
        app.assertNoRest()
        app.relaunch()
        app.assertNoRest()
        app.assertVibrationsPlayed(1)
    }

    /** Only the moment a row becomes confirmed counts, and a new rest replaces the one running. */
    @Test
    fun confirmingASetRunsARestWhileUncheckingOrEditingDoesNot() {
        repeatLast()
        app.enterDescribed("Squat set 1 reps", "6")
        app.assertNoRest()

        confirm("Squat set 1")
        app.assertVisible("Rest · Squat")
        app.assertRestRemaining(FRESH_DEFAULT_REST)
        app.tapDescription("Skip rest")
        app.assertNoRest()

        app.tapScrolledTo("Squat set 1 performed")
        app.assertChecked("Squat set 1 performed", false)
        app.assertNoRest()

        confirm("Squat set 1")
        app.moveClockForward(A_MINUTE)
        app.assertRestRemaining(A_MINUTE_INTO_DEFAULT_REST)
        app.enterDescribed("Squat set 1 reps", "7")
        app.assertRestRemaining(A_MINUTE_INTO_DEFAULT_REST)

        confirm("Squat set 2")
        app.assertRestRemaining(FRESH_DEFAULT_REST)

        // A skipped rest never ends, and the alarms replaced or dropped along the way never fire.
        app.tapDescription("Skip rest")
        app.moveClockForward(LONG_PAST_ANY_REST)
        app.assertNoRest()
        app.assertVibrationsPlayed(0)
    }

    @Test
    fun anAdjustedDurationIsRememberedForThatExerciseAlone() {
        repeatLast()
        app.tapScrolledTo("More rest for Squat")
        assertCardSays("Squat", "Rest 2:45")
        app.tapScrolledTo("More rest for Squat")
        assertCardSays("Squat", "Rest 3:00")
        confirm("Squat set 1")
        app.assertRestRemaining(150L..180L)

        // Adjusting the running rest moves its deadline and is remembered the same way.
        app.tapDescription("Less rest")
        assertCardSays("Squat", "Rest 2:45")
        app.assertRestRemaining(135L..165L)
        app.tapScrolledTo("Less rest for Squat")
        assertCardSays("Squat", "Rest 2:30")
        app.assertRestRemaining(120L..150L)
        app.tapDescription("More rest")
        assertCardSays("Squat", "Rest 2:45")

        app.goToExercise("Pull-Up")
        assertCardSays("Pull-Up", "Rest 2:30")
        confirm("Pull-Up set 1")
        app.assertRestRemaining(FRESH_DEFAULT_REST)

        app.relaunch()
        resume()
        app.goToExercise("Squat")
        assertCardSays("Squat", "Rest 2:45")

        // A later Workout on the same Exercise keeps it too.
        app.tapBelow("Discard Workout")
        app.tap("Discard")
        repeatLast()
        assertCardSays("Squat", "Rest 2:45")
        app.tapBelow("Complete Exercise")
        app.assertRestRemaining(135L..165L)
    }

    @Test
    fun turningRestOffAppliesToThatExerciseUntilTurnedBackOn() {
        repeatLast()
        confirm("Squat set 1")
        app.tapDescription("Turn off rest for Squat")
        app.assertNoRest()
        assertCardSays("Squat", "Rest off")
        app.assertSwitchedOn("Rest timer for Squat", false)

        // Logging carries on without a rest, and the choice outlives the process.
        confirm("Squat set 2")
        app.assertNoRest()
        app.relaunch()
        resume()
        app.goToExercise("Squat")
        assertCardSays("Squat", "Rest off")
        confirm("Squat set 3")
        app.assertNoRest()
        app.moveClockForward(LONG_PAST_ANY_REST)
        app.assertVibrationsPlayed(0)

        app.goToExercise("Pull-Up")
        confirm("Pull-Up set 1")
        app.assertVisible("Rest · Pull-Up")
        app.assertRestRemaining(FRESH_DEFAULT_REST)

        // Turning one Exercise's rest off leaves another's running; back on, it runs its duration again.
        app.goToExercise("Squat")
        app.tapScrolledTo("Rest timer for Squat")
        app.assertSwitchedOn("Rest timer for Squat", true)
        assertCardSays("Squat", "Rest 2:30")
        app.assertVisible("Rest · Pull-Up")
        app.tapScrolledTo("Squat set 3 performed")
        app.assertChecked("Squat set 3 performed", false)
        confirm("Squat set 3")
        app.assertVisible("Rest · Squat")
    }

    /** The form has no tabs under the bar, and its Save must stay reachable and working above it. */
    @Test
    fun theBarLeavesACorrectingFormUsable() {
        repeatLast()
        app.tapBelow("Complete Exercise")
        app.assertRestRemaining(FRESH_DEFAULT_REST)
        app.pressBackButton()
        app.tapBelow("History")
        app.tap("Usual session")
        app.tap("Edit Workout")
        app.assertVisible("Rest · Squat")
        app.assertNoBottomTabs()

        app.enterDescribed("Squat set 1 reps", "6")
        app.tapBelow("Save Changes")
        app.assertVisible("Rest · Squat")
        app.assertVisible("Usual session")
        app.assertTextContaining("6 reps")
        app.assertRestRemaining(1L..FRESH_DEFAULT_REST.last)
    }

    /** A force-stop takes the alarm with it, so the restart has to put it back from the stored deadline. */
    @Test
    fun aRestartContinuesTheDeadlineInsteadOfStartingOver() {
        repeatLast()
        app.tapBelow("Complete Exercise")
        app.moveClockForward(A_MINUTE)
        app.assertRestRemaining(A_MINUTE_INTO_DEFAULT_REST)

        app.relaunch()
        app.assertVisible("Rest · Squat")
        app.assertRestRemaining(A_MINUTE_INTO_DEFAULT_REST)
        app.assertVibrationsPlayed(0)

        app.moveClockPastRestDeadline()
        app.assertVibrationsPlayed(1)
        app.relaunch()
        app.assertVisible("Rest over")
        app.assertVibrationsPlayed(1)
    }

    /** Android's alarm brings the app back to end the rest, with nothing of it running. */
    @Test
    fun aRestEndingWithTheProcessGoneStillVibratesOnce() {
        repeatLast()
        app.tapBelow("Complete Exercise")
        app.assertRestRemaining(FRESH_DEFAULT_REST)

        app.killInBackground()
        app.moveClockForward(FRESH_DEFAULT_REST.last + 1)
        app.assertVibrationsPlayed(1)

        app.reopen()
        app.assertVisible("Rest over")
        app.assertVibrationsPlayed(1)
    }

    /** Nothing could vibrate on time, and opening the app long after is no moment to start. */
    @Test
    fun aRestThatEndedWhileForceStoppedIsShownOverWithoutVibrating() {
        repeatLast()
        app.tapBelow("Complete Exercise")
        app.assertRestRemaining(FRESH_DEFAULT_REST)

        app.forceStop()
        app.moveClockForward(LONG_PAST_ANY_REST)
        app.reopen()

        app.assertVisible("Rest over")
        app.assertVibrationsPlayed(0)
    }

    @Test
    fun endingTheWorkoutEndsItsRest() {
        repeatLast()
        app.tapBelow("Complete Exercise")
        app.assertRestRemaining(FRESH_DEFAULT_REST)
        app.tapBelow("Discard Workout")
        app.tap("Discard")
        app.assertNoRest()

        repeatLast()
        app.tapBelow("Complete Exercise")
        app.goToExercise("Pull-Up")
        app.tapBelow("Complete Exercise")
        app.assertRestRemaining(FRESH_DEFAULT_REST)
        app.tapBelow("Finish Workout")
        app.assertVisible("History")
        app.assertNoRest()

        app.moveClockForward(LONG_PAST_ANY_REST)
        app.assertVibrationsPlayed(0)
    }

    /**
     * Sync can find that the Exercise being rested from is one this device already holds under another
     * name; the two local rows become one, and neither the rest nor its remembered duration is lost.
     */
    @Test
    fun mergingTheExerciseKeepsItsRestAndRememberedDuration() {
        api.putExercise(FixtureExercise(PULL_UP, RING_ROW, "calisthenics"))
        app.setWifi(false)
        app.tapBelow("Library")
        app.assertVisible("Search Exercises")
        app.tap("Create Exercise")
        app.enter("Name", RING_ROW)
        app.tap("Calisthenics")
        app.tap("Save Exercise")
        app.assertVisible(RING_ROW)
        app.pressBackButton()

        app.tap("Start Empty")
        app.assertVisible("Active Workout")
        app.tapBelow("Add Exercise")
        app.assertVisible("Search Exercises")
        app.tap(RING_ROW)
        app.tapScrolledTo("More rest for $RING_ROW")
        assertCardSays(RING_ROW, "Rest 2:45")
        app.enterDescribed("$RING_ROW set 1 reps", "8")
        confirm("$RING_ROW set 1")
        app.assertVisible("Rest · $RING_ROW")

        app.setWifi(true)
        app.pressBackButton()
        app.assertEventuallyContaining("OpenHome already has an Exercise with this name")
        app.tapTextContaining("OpenHome already has an Exercise with this name")
        app.tap("Use OpenHome's Exercise")
        app.assertEventuallyVisible("Synced with OpenHome")

        app.assertVisible("Rest · $RING_ROW")
        resume()
        assertCardSays(RING_ROW, "Rest 2:45")
        app.moveClockPastRestDeadline()
        app.assertVibrationsPlayed(1)
    }
}
