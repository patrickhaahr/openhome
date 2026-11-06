package com.patrickhaahr.openhome.e2e

import org.junit.After
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test

/** System back through the real activity, with Android delivering the back event. */
class BackNavigationTest {
    private val app = AppDriver()
    private val api =
        FakeAxum(
            exercises = mapOf(1L to "Pull-Up"),
            workouts =
                listOf(
                    FixtureWorkout(
                        id = 1,
                        date = "2026-03-02",
                        name = "Pull day",
                        entries = listOf(FixtureEntry(exerciseId = 1, sets = listOf(FixtureSet(1, reps = 8)))),
                    ),
                ),
        )

    @Before
    fun setUpApp() {
        app.clearAppData()
        app.relaunch()
        app.completeSetup(api)
    }

    @After
    fun stopApi() {
        api.close()
    }

    private fun openSavedWorkoutFromHistory() {
        app.tap("History")
        app.tap("Pull day")
        app.assertVisible("8 reps · Bodyweight")
    }

    private fun assertOnHistory() {
        app.assertVisible("History")
        app.assertVisible("1 Workout on this device")
        app.assertVisible("Pull day")
        app.assertAbsent("8 reps · Bodyweight")
    }

    @Test
    fun backButtonFromSavedWorkoutReturnsToHistoryThenWorkoutThenLeavesApp() {
        openSavedWorkoutFromHistory()

        app.pressBackButton()
        assertOnHistory()

        app.pressBackButton()
        app.assertOnWorkoutTab()

        app.pressBackButton()
        app.assertLeftApp()
    }

    @Test
    fun backGestureFromSavedWorkoutReturnsToHistory() {
        assumeTrue("Device uses three-button navigation", app.usesGestureNavigation())
        openSavedWorkoutFromHistory()

        app.swipeBackGesture()
        assertOnHistory()
    }

    @Test
    fun tabsAreWorkoutThenHomeAndBackFromHomeReturnsToWorkout() {
        app.assertBottomTabs("Workout", "Home", selected = "Workout")

        app.tap("Home")
        app.assertVisible("Full TV remote")
        app.assertBottomTabs("Workout", "Home", selected = "Home")

        app.pressBackButton()
        app.assertOnWorkoutTab()
    }

    @Test
    fun profileOpensFromAvatarOutsideTheTabsAndBackReturnsToItsOrigin() {
        app.tap("Home")
        app.tapDescription("Open Profile")
        app.assertVisible("Max Heart Rate")
        app.assertNoBottomTabs()

        app.pressBackButton()
        app.assertVisible("Full TV remote")

        app.pressBackButton()
        app.assertOnWorkoutTab()
    }

    @Test
    fun switchingTabsKeepsHistoryOpenInTheWorkoutTab() {
        app.tap("History")
        app.assertVisible("Pull day")

        app.tap("Home")
        app.assertVisible("Full TV remote")
        app.tap("Workout")

        app.assertVisible("Pull day")
    }
}
