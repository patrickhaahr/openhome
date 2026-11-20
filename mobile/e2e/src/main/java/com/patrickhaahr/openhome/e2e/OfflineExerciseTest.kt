package com.patrickhaahr.openhome.e2e

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

private const val WAITING_FOR_WIFI = "Saved on device · Waiting for Wi-Fi"
private const val WAITING_FOR_EXERCISE = "Saved on device · Waiting for its Exercise to sync"
private const val RING_ROW = "Ring Row"
private const val SETTLED_MS = 4_000L
private val LIBRARY =
    listOf(
        FixtureExercise(1, "Pull-Up", "calisthenics", "back"),
        FixtureExercise(2, "Back Squat", "gym", "legs", "barbell"),
        FixtureExercise(3, "Planche Hold", "calisthenics"),
        FixtureExercise(4, "Lat Pulldown", "gym", "Back", "cable"),
    )
private val LIBRARY_NAMES = LIBRARY.map { it.name }

class OfflineExerciseTest {
    private val app = AppDriver()
    private val api = FakeAxum(exerciseDetails = LIBRARY)

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

    private fun openLibrary() {
        app.tapBelow("Library")
        app.assertVisible("Search Exercises")
    }

    private fun fillExercise(name: String, category: String, muscleGroup: String = "", equipment: String = "") {
        app.enter("Name", name)
        app.tap(category)
        if (muscleGroup.isNotEmpty()) app.enter("Muscle group (optional)", muscleGroup)
        if (equipment.isNotEmpty()) app.enter("Equipment (optional)", equipment)
    }

    private fun logWorkout(workoutName: String, exercise: String) {
        app.tap("Start Empty")
        app.assertVisible("Active Workout")
        app.tap("Add name or notes")
        app.enter("Name (optional)", workoutName)
        app.tap("Done")
        app.tapBelow("Add Exercise")
        app.assertVisible("Search Exercises")
        app.tap(exercise)
        logOneSet(exercise)
        app.tapBelow("Finish Workout")
        app.assertVisible(workoutName)
        app.assertVisible("History")
    }

    private fun logOneSet(exercise: String) {
        app.enterDescribed("$exercise set 1 reps", "8")
        app.tapScrolledTo("$exercise set 1 performed")
        app.assertChecked("$exercise set 1 performed", true)
    }

    private fun createRingRowOffline() {
        openLibrary()
        app.tap("Create Exercise")
        fillExercise(RING_ROW, "Calisthenics", muscleGroup = "back", equipment = "rings")
        app.tap("Save Exercise")
        app.assertVisible(RING_ROW)
        app.pressBackButton()
    }

    private fun storedNamed(name: String): List<FixtureExercise> = api.storedExercises().filter { it.name == name }

    private fun workoutNamed(name: String): FixtureWorkout = api.storedWorkouts().single { it.name == name }

    @Test
    fun libraryIsBrowsedSearchedAndFilteredOfflineByItsOwnMetadata() {
        app.setWifi(false)
        openLibrary()
        app.assertExercisesShown(*LIBRARY_NAMES.toTypedArray(), among = LIBRARY_NAMES)
        app.assertVisible("gym · legs · barbell")
        app.assertVisible("calisthenics")

        app.tap("Gym")
        app.assertExercisesShown("Back Squat", "Lat Pulldown", among = LIBRARY_NAMES)
        app.tap("Back")
        app.assertExercisesShown("Lat Pulldown", among = LIBRARY_NAMES)
        app.tap("Gym")
        app.assertExercisesShown("Pull-Up", "Lat Pulldown", among = LIBRARY_NAMES)
        app.tap("Back")

        app.enter("Search Exercises", "barbell")
        app.assertExercisesShown("Back Squat", among = LIBRARY_NAMES)
        app.enter("Search Exercises", "planche")
        app.assertExercisesShown("Planche Hold", among = LIBRARY_NAMES)
        app.tap("Gym")
        app.assertVisible("No matching Exercises")
        app.tap("Clear search and filters")
        app.assertExercisesShown(*LIBRARY_NAMES.toTypedArray(), among = LIBRARY_NAMES)

        assertEquals(emptyList<String>(), api.exerciseDeliveries())
    }

    @Test
    fun anExerciseIsValidatedBeforeItIsSaved() {
        app.setWifi(false)
        openLibrary()
        app.tap("Create Exercise")

        app.tap("Save Exercise")
        app.assertVisible("Enter a name")

        app.enter("Name", "   ")
        app.tap("Save Exercise")
        app.assertVisible("Enter a name")

        app.enter("Name", "Ring Dip")
        app.tap("Save Exercise")
        app.assertVisible("Choose a category")

        app.enter("Name", "  pull-up ")
        app.tap("Gym")
        app.tap("Save Exercise")
        app.assertVisible("pull-up is already in your Library")

        app.pressBackButton()
        app.assertAbsent("Ring Dip")
        app.assertVisible("Pull-Up")
    }

    @Test
    fun exerciseCreatedOfflineIsUsableAtOnceAndSyncsBeforeItsWorkoutOnWifi() {
        app.setWifi(false)

        createRingRowOffline()
        app.assertVisible("5 Exercises in your Library")

        app.tap("Start Empty")
        app.assertVisible("Active Workout")
        app.tap("Add name or notes")
        app.enter("Name (optional)", "Basement session")
        app.tap("Done")
        app.tapBelow("Add Exercise")
        app.enter("Search Exercises", "Sissy Squat")
        app.assertVisible("No matching Exercises")
        app.tap("Create Exercise")
        app.tap("Gym")
        app.tap("Save and add to Workout")
        logOneSet("Sissy Squat")
        app.pressBackButton()
        app.tap("Resume Workout")

        app.tapBelow("Add Exercise")
        app.enter("Search Exercises", "ring")
        app.tap(RING_ROW)
        logOneSet(RING_ROW)

        app.relaunch()
        app.assertVisible("6 Exercises in your Library")
        app.tap("Resume Workout")
        app.assertChecked("$RING_ROW set 1 performed", true)
        app.goToExercise("Sissy Squat")
        app.assertChecked("Sissy Squat set 1 performed", true)

        app.tapBelow("Finish Workout")
        app.assertVisible(WAITING_FOR_WIFI)
        app.assertTextContaining("Sissy Squat · $RING_ROW · 2 Sets")
        app.relaunch()
        openLibrary()
        app.assertVisible("gym")
        app.enter("Search Exercises", "ring row")
        app.assertVisible("calisthenics · back · rings")
        app.assertVisible(WAITING_FOR_WIFI)
        app.sleepFor(SETTLED_MS)
        assertEquals("Mobile data must not send anything", emptyList<String>(), api.exerciseDeliveries())
        assertEquals("Mobile data must not send anything", emptyList<String>(), api.workoutDeliveries())

        app.setWifi(true)
        app.pressBackButton()
        app.assertEventuallyVisible("Synced with OpenHome")

        val ringRow = storedNamed(RING_ROW).single()
        assertEquals("calisthenics", ringRow.category)
        assertEquals("back", ringRow.muscleGroup)
        assertEquals("rings", ringRow.equipment)
        val sissySquat = storedNamed("Sissy Squat").single()
        assertEquals("gym", sissySquat.category)
        assertEquals(null, sissySquat.muscleGroup)
        assertEquals(
            listOf("exercise:$RING_ROW", "exercise:Sissy Squat", "workout:Basement session"),
            api.commitOrder(),
        )
        assertEquals(
            listOf(sissySquat.id, ringRow.id),
            workoutNamed("Basement session").entries.map { it.exerciseId },
        )

        app.relaunch()
        app.tapBelow("History")
        app.tap("Basement session")
        app.assertVisible(RING_ROW)
        app.assertVisible("Sissy Squat")
        assertEquals(2, api.exerciseDeliveries().size)
        assertEquals(1, api.workoutDeliveries().size)
    }

    @Test
    fun lostExerciseResponseRetriesAsOneExerciseAndItsWorkoutFollowsOnceAfterARestart() {
        api.loseExerciseResponses()
        api.failWorkoutCreates()
        openLibrary()
        app.tap("Create Exercise")
        fillExercise(RING_ROW, "Calisthenics")
        app.tap("Save Exercise")
        app.pressBackButton()
        logWorkout("Lossy session", RING_ROW)
        app.pressBackButton()

        app.assertEventuallyVisible("Saved on device · Can't reach OpenHome, will retry")
        assertEquals(1, storedNamed(RING_ROW).size)
        assertEquals("Nothing depends on an unanswered Exercise", emptyList<String>(), api.workoutDeliveries())

        api.healthy()
        api.failWorkoutCreates()
        app.relaunch()
        app.assertEventuallyVisible("Saved on device · OpenHome had a problem, will retry")
        assertEquals(1, storedNamed(RING_ROW).size)
        assertEquals("Each delivery is the same logical Exercise", 1, api.exerciseDeliveries().toSet().size)
        assertTrue("The Exercise should have been retried", api.exerciseDeliveries().size >= 2)
        val exerciseDeliveries = api.exerciseDeliveries().size

        api.healthy()
        app.relaunch()
        app.assertEventuallyVisible("Synced with OpenHome")
        assertEquals(1, storedNamed(RING_ROW).size)
        assertEquals("An acknowledged Exercise is not sent again", exerciseDeliveries, api.exerciseDeliveries().size)
        assertEquals(listOf(storedNamed(RING_ROW).single().id), workoutNamed("Lossy session").entries.map { it.exerciseId })
    }

    @Test
    fun refusedExerciseKeepsItsWorkoutQueuedLetsOthersSyncAndRenamingDeliversBoth() {
        api.putExercise(FixtureExercise(50, RING_ROW, "gym", "back"))
        app.setWifi(false)
        createRingRowOffline()
        logWorkout("Blocked session", RING_ROW)
        app.pressBackButton()
        logWorkout("Free session", "Pull-Up")
        app.setWifi(true)
        app.pressBackButton()

        app.assertEventuallyContaining("OpenHome already has an Exercise with this name")
        waitUntilStored("Free session")
        assertEquals(listOf("Free session"), api.storedWorkouts().map { it.name })
        assertEquals(1, api.workoutDeliveries().size)

        app.relaunch()
        app.assertVisible("2 Workouts on this device")
        app.tapBelow("History")
        app.assertVisible(WAITING_FOR_EXERCISE)
        app.tap("Blocked session")
        app.assertVisible("8 reps · Bodyweight")
        app.pressBackButton()
        app.pressBackButton()
        val attempts = api.exerciseDeliveries().size
        app.sleepFor(SETTLED_MS)
        assertEquals("A refused change waits for the user", attempts, api.exerciseDeliveries().size)

        app.tapTextContaining("OpenHome already has an Exercise with this name")
        app.assertVisible("Fix Exercise")
        app.enter("Name", "Pull-Up")
        app.tap("Save and sync again")
        app.assertVisible("Pull-Up is already in your Library")
        app.enter("Name", "Ring Row Wide")
        app.tap("Save and sync again")

        app.assertEventuallyVisible("Synced with OpenHome")
        val renamed = storedNamed("Ring Row Wide").single()
        assertEquals(listOf(renamed.id), workoutNamed("Blocked session").entries.map { it.exerciseId })
        assertEquals("The other client's Exercise stays as it was", "gym", storedNamed(RING_ROW).single().category)
        app.tapBelow("History")
        app.tap("Blocked session")
        app.assertVisible("Ring Row Wide")
    }

    @Test
    fun takenNameCanUseOpenHomesExerciseWithoutADuplicate() {
        api.putExercise(FixtureExercise(50, RING_ROW, "gym", "back", "rings"))
        val exercisesBefore = api.storedExercises()
        app.setWifi(false)
        createRingRowOffline()
        logWorkout("Linked session", RING_ROW)
        app.setWifi(true)
        app.pressBackButton()

        app.assertEventuallyContaining("OpenHome already has an Exercise with this name")
        app.tapTextContaining("OpenHome already has an Exercise with this name")
        app.tap("Use OpenHome's Exercise")

        app.assertEventuallyVisible("Synced with OpenHome")
        assertEquals("Nothing new in the Library", exercisesBefore, api.storedExercises())
        assertEquals(listOf(50L), workoutNamed("Linked session").entries.map { it.exerciseId })
        openLibrary()
        app.enter("Search Exercises", "ring")
        app.assertVisible("gym · back · rings")
        app.assertAbsent(WAITING_FOR_WIFI)
    }

    @Test
    fun usingOpenHomesExerciseMergesIntoTheLocalRowThatAlreadyHoldsIt() {
        api.putExercise(FixtureExercise(1, RING_ROW, "calisthenics", "back"))
        app.setWifi(false)
        createRingRowOffline()
        logWorkout("Merged session", RING_ROW)
        app.setWifi(true)
        app.pressBackButton()

        app.assertEventuallyContaining("OpenHome already has an Exercise with this name")
        app.tapTextContaining("OpenHome already has an Exercise with this name")
        app.tap("Use OpenHome's Exercise")

        app.assertEventuallyVisible("Synced with OpenHome")
        assertEquals(listOf(1L), workoutNamed("Merged session").entries.map { it.exerciseId })
        app.assertVisible("4 Exercises in your Library")
        app.tapBelow("History")
        app.tap("Merged session")
        app.assertVisible(RING_ROW)
    }

    private fun waitUntilStored(workoutName: String) {
        repeat(STORED_POLLS) {
            if (api.storedWorkouts().any { it.name == workoutName }) return
            app.sleepFor(STORED_POLL_MS)
        }
        error("Axum never received $workoutName")
    }
}

private const val STORED_POLLS = 90
private const val STORED_POLL_MS = 1_000L
