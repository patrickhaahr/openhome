package com.patrickhaahr.openhome.ui.workout

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.ActiveWorkoutRepository
import com.patrickhaahr.openhome.fitness.Exercise
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.ui.appViewModel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

private class ExercisePickerViewModel(
    fitness: FitnessRepository,
    private val activeWorkouts: ActiveWorkoutRepository,
    private val saveScope: CoroutineScope,
) : ViewModel() {
    val exercises: StateFlow<LibraryLookup<Exercise>> =
        fitness.observeExercises()
            .map<List<Exercise>, LibraryLookup<Exercise>> { LibraryLookup.Loaded(it) }
            .catch { emit(LibraryLookup.Unreadable) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), LibraryLookup.Loading)

    private var adding = false

    /** One Exercise per visit, so a second tap before the screen leaves cannot add it twice. */
    fun add(exercise: Exercise, onAdded: () -> Unit) {
        if (adding) return
        adding = true
        saveScope.launch {
            activeWorkouts.addExercise(exercise.id)
            onAdded()
        }
    }
}

/** Everything in the Library, including Exercises not yet synced, so choosing one never needs a connection. */
@Composable
fun ExercisePickerRoute(onPicked: () -> Unit, onCreate: (String) -> Unit) {
    val viewModel = appViewModel { ExercisePickerViewModel(it.fitness, it.activeWorkout, it.saveScope) }
    val exercises by viewModel.exercises.collectAsStateWithLifecycle()
    ExerciseBrowser(
        title = "Add Exercise",
        lookup = exercises,
        exerciseOf = { it },
        onCreate = onCreate,
        row = { exercise ->
            ExerciseRow(exercise, onClick = { viewModel.add(exercise, onPicked) }, onClickLabel = "Add to Workout")
        },
    )
}
