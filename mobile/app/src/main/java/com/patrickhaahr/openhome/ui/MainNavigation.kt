package com.patrickhaahr.openhome.ui

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import androidx.navigation.NavDestination
import androidx.navigation.NavDestination.Companion.hasRoute
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.navigation
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.Motion
import com.patrickhaahr.openhome.ui.design.pressable
import com.patrickhaahr.openhome.ui.home.AdvancedBacklightScreen
import com.patrickhaahr.openhome.ui.home.HomeScreen
import com.patrickhaahr.openhome.ui.home.TvRemoteScreen
import com.patrickhaahr.openhome.ui.profile.EditProfileRoute
import com.patrickhaahr.openhome.ui.profile.ProfileRoute
import com.patrickhaahr.openhome.ui.profile.RecordBodyWeightRoute
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.workout.ActiveWorkoutRoute
import com.patrickhaahr.openhome.ui.workout.CorrectionExercisePickerRoute
import com.patrickhaahr.openhome.ui.workout.ExercisePickerRoute
import com.patrickhaahr.openhome.ui.workout.FixExerciseRoute
import com.patrickhaahr.openhome.ui.workout.HistoryRoute
import com.patrickhaahr.openhome.ui.workout.LibraryRoute
import com.patrickhaahr.openhome.ui.workout.NewExerciseRoute
import com.patrickhaahr.openhome.ui.workout.RestTimerBar
import com.patrickhaahr.openhome.ui.workout.WorkoutCorrectionRoute
import com.patrickhaahr.openhome.ui.workout.WorkoutDetailRoute
import com.patrickhaahr.openhome.ui.workout.WorkoutRoute
import kotlinx.serialization.Serializable

private const val TRANSITION_MILLIS = 220
private val RISE = 24.dp
private const val PICKED_EXERCISE = "pickedExercise"

@Serializable
private data object WorkoutTab

@Serializable
private data object WorkoutHome

@Serializable
private data object History

@Serializable
private data class WorkoutDetail(val id: Long)

@Serializable
private data class WorkoutCorrection(val id: Long)

@Serializable
private data object CorrectionExercisePicker

@Serializable
private data object ActiveWorkoutDestination

@Serializable
private data object ExercisePicker

@Serializable
private data object Library

/** [addToWorkout] when opened from the picker: the new Exercise goes straight into the session. */
@Serializable
private data class NewExercise(val initialName: String, val addToWorkout: Boolean)

@Serializable
private data class FixExercise(val id: Long)

@Serializable
private data object HomeTab

@Serializable
private data object HomeRoot

@Serializable
private data object AdvancedBacklight

@Serializable
private data object TvRemote

@Serializable
private data object Profile

@Serializable
private data object EditProfile

@Serializable
private data object RecordBodyWeight

private enum class Tab(val graph: Any, val root: Any, val label: String, val glyph: Glyph) {
    Workout(WorkoutTab, WorkoutHome, "Workout", Glyph.Barbell),
    Home(HomeTab, HomeRoot, "Home", Glyph.House),
}

private fun NavDestination?.isIn(tab: Tab): Boolean =
    this?.hierarchy?.any { destination ->
        when (tab) {
            Tab.Workout -> destination.hasRoute<WorkoutTab>()
            Tab.Home -> destination.hasRoute<HomeTab>()
        }
    } == true

/**
 * The app's navigation hierarchy on one back stack, so system back and the back gesture pop nested
 * views. Workout is the start destination: back at a tab root returns there, and back on Workout's
 * root leaves the activity to Android's default behavior. Destinations navigate onward only while
 * resumed, so a second tap during a transition cannot stack a duplicate.
 */
@Composable
fun MainNavigation() {
    val navController = rememberNavController()
    val backStackEntry by navController.currentBackStackEntryAsState()
    val destination = backStackEntry?.destination
    // An unsaved correction must not be one tab tap away from being dropped, so its form hides them.
    val correcting =
        destination?.hasRoute<WorkoutCorrection>() == true || destination?.hasRoute<CorrectionExercisePicker>() == true
    val inTab = !correcting && Tab.entries.any { destination.isIn(it) }
    val rise = with(LocalDensity.current) { RISE.roundToPx() }

    Scaffold(
        containerColor = Lit.Ground,
        bottomBar = {
            Column {
                // On every destination; without the tabs beneath, it keeps clear of the system's own bar itself.
                RestTimerBar(if (inTab) Modifier else Modifier.windowInsetsPadding(WindowInsets.navigationBars))
                if (inTab) {
                    BottomTabs(destination, onSelect = { tab -> navController.openTab(tab, destination) })
                }
            }
        },
    ) { padding ->
        NavHost(
            navController = navController,
            startDestination = WorkoutTab,
            modifier = Modifier.padding(padding),
            enterTransition = {
                fadeIn(tween(TRANSITION_MILLIS)) + slideInVertically(tween(TRANSITION_MILLIS, easing = Motion.Ease)) { rise }
            },
            exitTransition = { fadeOut(tween(TRANSITION_MILLIS / 2)) },
        ) {
            navigation<WorkoutTab>(startDestination = WorkoutHome) {
                composable<WorkoutHome> { entry ->
                    WorkoutRoute(
                        onOpenHistory = dropUnlessResumed { navController.navigate(History) },
                        onOpenLibrary = dropUnlessResumed { navController.navigate(Library) },
                        onFixExercise = { id ->
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                navController.navigate(FixExercise(id))
                            }
                        },
                        onOpenProfile = dropUnlessResumed { navController.navigate(Profile) },
                        onOpenActiveWorkout = {
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                navController.navigate(ActiveWorkoutDestination)
                            }
                        },
                    )
                }
                composable<ActiveWorkoutDestination> {
                    ActiveWorkoutRoute(
                        onAddExercise = dropUnlessResumed { navController.navigate(ExercisePicker) },
                        // Finishing and discarding replace this destination; back must not return to it.
                        onFinished = {
                            navController.navigate(History) { popUpTo(WorkoutHome) }
                        },
                        onDiscarded = { navController.popBackStack(WorkoutHome, inclusive = false) },
                    )
                }
                composable<ExercisePicker> { entry ->
                    ExercisePickerRoute(
                        onPicked = {
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                navController.popBackStack()
                            }
                        },
                        onCreate = { query ->
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                navController.navigate(NewExercise(query, addToWorkout = true))
                            }
                        },
                    )
                }
                composable<Library> { entry ->
                    LibraryRoute(
                        onCreate = { query ->
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                navController.navigate(NewExercise(query, addToWorkout = false))
                            }
                        },
                        onFix = { id ->
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                navController.navigate(FixExercise(id))
                            }
                        },
                    )
                }
                composable<NewExercise> { entry ->
                    val route = entry.toRoute<NewExercise>()
                    NewExerciseRoute(
                        initialName = route.initialName,
                        addToWorkout = route.addToWorkout,
                        onSaved = {
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                // From the picker, past it too: the Exercise is already in the session.
                                if (route.addToWorkout) {
                                    navController.popBackStack(ActiveWorkoutDestination, inclusive = false)
                                } else {
                                    navController.popBackStack()
                                }
                            }
                        },
                    )
                }
                composable<FixExercise> { entry ->
                    FixExerciseRoute(
                        exerciseId = entry.toRoute<FixExercise>().id,
                        onSaved = {
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                navController.popBackStack()
                            }
                        },
                    )
                }
                composable<History> { entry ->
                    HistoryRoute(
                        onOpenWorkout = { id ->
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                navController.navigate(WorkoutDetail(id))
                            }
                        },
                    )
                }
                composable<WorkoutDetail> { entry ->
                    WorkoutDetailRoute(
                        workoutId = entry.toRoute<WorkoutDetail>().id,
                        onEdit =
                            dropUnlessResumed {
                                navController.navigate(WorkoutCorrection(entry.toRoute<WorkoutDetail>().id))
                            },
                        // Back from the session returns to where sessions start, not to this Workout.
                        onOpenActiveWorkout = {
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                navController.navigate(ActiveWorkoutDestination) { popUpTo(WorkoutHome) }
                            }
                        },
                    )
                }
                composable<WorkoutCorrection> { entry ->
                    val picked by entry.savedStateHandle.getStateFlow<Long?>(PICKED_EXERCISE, null)
                        .collectAsStateWithLifecycle()
                    WorkoutCorrectionRoute(
                        workoutId = entry.toRoute<WorkoutCorrection>().id,
                        pickedExercise = picked,
                        onPickedExerciseUsed = { entry.savedStateHandle[PICKED_EXERCISE] = null },
                        onAddExercise = dropUnlessResumed { navController.navigate(CorrectionExercisePicker) },
                        onDone = { navController.popBackStack() },
                    )
                }
                composable<CorrectionExercisePicker> { entry ->
                    CorrectionExercisePickerRoute(
                        onPicked = { id ->
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                navController.previousBackStackEntry?.savedStateHandle?.set(PICKED_EXERCISE, id)
                                navController.popBackStack()
                            }
                        },
                    )
                }
            }
            navigation<HomeTab>(startDestination = HomeRoot) {
                composable<TvRemote> { TvRemoteScreen() }
                composable<AdvancedBacklight> { AdvancedBacklightScreen() }
                composable<HomeRoot> {
                    HomeScreen(
                        onOpenProfile = dropUnlessResumed { navController.navigate(Profile) },
                        onOpenRemote = dropUnlessResumed { navController.navigate(TvRemote) },
                        onOpenBacklight = dropUnlessResumed { navController.navigate(AdvancedBacklight) },
                    )
                }
            }
            composable<Profile> {
                ProfileRoute(
                    onEdit = dropUnlessResumed { navController.navigate(EditProfile) },
                    onRecordBodyWeight = dropUnlessResumed { navController.navigate(RecordBodyWeight) },
                )
            }
            composable<EditProfile> { entry ->
                EditProfileRoute(
                    onSaved = {
                        if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                            navController.popBackStack()
                        }
                    },
                )
            }
            composable<RecordBodyWeight> { entry ->
                RecordBodyWeightRoute(
                    onSaved = {
                        if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                            navController.popBackStack()
                        }
                    },
                )
            }
        }
    }
}

/** No labels: the barbell and the house say it. The chosen one is lit, and quiet enough not to outshine the work. */
@Composable
private fun BottomTabs(destination: NavDestination?, onSelect: (Tab) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .background(Lit.Ground)
            .windowInsetsPadding(WindowInsets.navigationBars)
            .padding(horizontal = ScreenMargin, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Tab.entries.forEach { tab ->
            val selected = destination.isIn(tab)
            val share by animateFloatAsState(if (selected) 1.7f else 1f, tween(Motion.PANEL, easing = Motion.Ease), label = "tab share")
            val fill by animateColorAsState(if (selected) Lit.Raised else Lit.Ground, tween(Motion.PANEL), label = "tab fill")
            val ink by animateColorAsState(if (selected) Lit.Lamp else Lit.Muted, tween(Motion.PANEL), label = "tab ink")
            val source = remember { MutableInteractionSource() }
            Box(
                Modifier
                    .weight(share)
                    .height(60.dp)
                    .pressable(source, depth = 0.97f)
                    .clip(CircleShape)
                    .background(fill)
                    .selectable(
                        selected = selected,
                        onClick = { onSelect(tab) },
                        role = Role.Tab,
                        interactionSource = source,
                        indication = LocalIndication.current,
                    ).semantics { contentDescription = tab.label },
                contentAlignment = Alignment.Center,
            ) {
                GlyphIcon(tab.glyph, tint = ink, size = 28.dp)
            }
        }
    }
}

/** Keeps each tab's nested state; choosing the tab already open returns to its root. */
private fun NavHostController.openTab(tab: Tab, current: NavDestination?) {
    if (current.isIn(tab)) {
        popBackStack(tab.root, inclusive = false)
        return
    }
    navigate(tab.graph) {
        popUpTo(graph.findStartDestination().id) { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
}
