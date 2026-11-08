package com.patrickhaahr.openhome.e2e

import java.time.LocalDate
import org.junit.After
import org.junit.Before
import org.junit.Test

/** Setup Flow outcomes through the real app, its real database and a real process relaunch. */
class SetupFlowTest {
    private val app = AppDriver()
    private var api: FakeAxum? = null

    @Before
    fun startFromFreshInstall() {
        app.clearAppData()
        app.relaunch()
        app.assertOnSetup()
    }

    @After
    fun stopApi() {
        api?.close()
    }

    private fun serve(fixture: FakeAxum): FakeAxum = fixture.also { api = it }

    @Test
    fun rejectedConnectionsStayOnSetup() {
        val api = serve(FakeAxum())

        app.submitSetup(baseUrl = "not a url", apiKey = API_KEY)
        app.assertVisible("Base URL must start with http:// or https://.")

        app.submitSetup(baseUrl = "http://127.0.0.1:1", apiKey = API_KEY)
        app.assertTextContaining("Couldn't reach OpenHome")

        app.submitSetup(baseUrl = api.baseUrl, apiKey = "wrong-key")
        app.assertVisible("OpenHome rejected that API Key.")
        app.assertAbsent("Last Workout")

        app.relaunch()
        app.assertOnSetup()
    }

    @Test
    fun wholeHistoryAndBodyWeightStayAvailableOfflineAfterRelaunch() {
        val workouts = manyWorkouts(count = 620)
        val api =
            serve(
                FakeAxum(
                    exercises = mapOf(1L to "Pull-Up", 2L to "Planche Hold"),
                    workouts = workouts,
                    bodyWeight = manyBodyWeights(count = 540),
                    profile = FixtureProfile(178.5, "male", 192, "1994-03-17"),
                ),
            )
        app.completeSetup(api)
        app.assertVisible("620 Workouts on this device")
        app.assertVisible("2 Exercises in your Library")

        // The API is gone from here on, and the process restarted, so all of this comes from storage.
        api.close()
        app.relaunch()
        app.assertOnWorkoutTab()
        app.assertAbsent("Connect to OpenHome")

        app.tap("History")
        app.assertVisible("620 Workouts on this device")
        val oldest = workouts.minBy { it.id }
        app.scrollDownUntilVisible(oldest.name!!)
        app.tap(oldest.name!!)
        app.assertVisible("Pull-Up")
        app.assertVisible("8 reps · Bodyweight")
        app.assertVisible("6 reps · +12.5 kg")
        app.assertVisible("RPE 9")
        // A Timed Hold shows its duration and no reps.
        app.assertVisible("20 s · Bodyweight")

        app.pressBackButton()
        app.pressBackButton()
        app.assertOnWorkoutTab()
        app.tapDescription("Open Profile")
        app.assertVisible("178.5 cm")
        app.assertVisible("Male")
        app.assertVisible("192 bpm")
        app.assertVisible("Saved on this device")
        // The oldest weigh-in is beyond every default page of the list endpoint.
        app.scrollDownUntilVisible("Mon, 1 Jan 2024")
    }

    @Test
    fun failedDownloadSavesNothingAndRetrySucceeds() {
        val workouts = manyWorkouts(count = 30)
        val api = serve(FakeAxum(mapOf(1L to "Pull-Up", 2L to "Planche Hold"), workouts))
        api.failWorkoutDetailsAfter(successes = 10)

        app.submitSetup(api.baseUrl, API_KEY)
        app.assertTextContaining("Download stopped, so nothing was saved.")
        app.assertAbsent("Last Workout")

        // Trying again while the API still fails ends in the same failure, not in a stuck download.
        app.submitSetup(api.baseUrl, API_KEY)
        app.assertTextContaining("Download stopped, so nothing was saved.")
        app.assertAbsent("Last Workout")

        // The failure survives a restart as an unfinished Setup Flow, not as a finished one.
        app.relaunch()
        app.assertOnSetup()
        app.assertTextContaining("The last download didn't finish")

        api.healthy()
        app.completeSetup(api)
        app.assertVisible("30 Workouts on this device")
    }

    @Test
    fun processKilledDuringDownloadDoesNotCompleteSetup() {
        val api = serve(FakeAxum(mapOf(1L to "Pull-Up", 2L to "Planche Hold"), manyWorkouts(count = 5)))
        val release = api.holdWorkoutDetails()

        app.submitSetup(api.baseUrl, API_KEY)
        api.awaitHeldDetailRequest()

        app.relaunch()
        app.assertOnSetup()
        app.assertTextContaining("The last download didn't finish")

        release.countDown()
        app.completeSetup(api)
        app.assertVisible("5 Workouts on this device")
    }

    @Test
    fun emptyAccountCompletesSetupWithEmptyStates() {
        val api = serve(FakeAxum())

        app.completeSetup(api)
        app.assertVisible("No Workouts yet")
        app.assertVisible("0 Workouts on this device")

        app.tap("History")
        app.assertVisible("Workouts you log will appear here.")
        app.pressBackButton()
        app.assertOnWorkoutTab()

        app.tapDescription("Open Profile")
        app.assertVisible("Not set")
        app.assertVisible("No Body Weight recorded yet.")
    }
}

/**
 * Workouts newest-numbered last; every tenth shares its date with the previous one, so that the
 * server's page boundaries can fall inside a day.
 */
private fun manyWorkouts(count: Int): List<FixtureWorkout> {
    var date = LocalDate.of(2024, 1, 1)
    return (1..count).map { number ->
        if (number % 10 != 0) date = date.plusDays(1)
        FixtureWorkout(
            id = number.toLong(),
            date = date.toString(),
            name = "Workout %03d".format(number),
            entries =
                listOf(
                    FixtureEntry(
                        exerciseId = 1,
                        sets =
                            listOf(
                                FixtureSet(1, reps = 8, rpe = 7),
                                FixtureSet(2, reps = 6, weightKg = 12.5, rpe = 9),
                            ),
                    ),
                    FixtureEntry(exerciseId = 2, sets = listOf(FixtureSet(1, durationSeconds = 20))),
                ),
        )
    }
}

private fun manyBodyWeights(count: Int): List<FixtureBodyWeight> =
    (0 until count).map { day ->
        FixtureBodyWeight(day + 1L, LocalDate.of(2024, 1, 1).plusDays(day.toLong()).toString(), 80.0)
    }
