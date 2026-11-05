package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.ActiveWorkoutRepository
import com.patrickhaahr.openhome.fitness.Exercise
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.appViewModel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

private class ExercisePickerViewModel(
    fitness: FitnessRepository,
    private val activeWorkouts: ActiveWorkoutRepository,
    private val saveScope: CoroutineScope,
) : ViewModel() {
    val exercises: StateFlow<List<Exercise>?> =
        fitness.observeExercises().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

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

/** Downloaded Exercises only, so choosing one never needs a connection. */
@Composable
fun ExercisePickerRoute(onPicked: () -> Unit) {
    val viewModel = appViewModel { ExercisePickerViewModel(it.fitness, it.activeWorkout, it.saveScope) }
    val exercises by viewModel.exercises.collectAsStateWithLifecycle()
    var query by rememberSaveable { mutableStateOf("") }
    val matches = exercises?.filter { it.name.contains(query.trim(), ignoreCase = true) }

    Column(Modifier.fillMaxSize().imePadding()) {
        ScreenHeader(title = "Add Exercise")
        OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            label = { Text("Search Exercises") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
        )
        when {
            matches == null -> Unit
            matches.isEmpty() -> EmptyState(title = "No matching Exercises", message = "Try a different name.")
            else ->
                LazyColumn(contentPadding = PaddingValues(vertical = 8.dp)) {
                    items(matches, key = { it.id }) { exercise ->
                        ExerciseRow(exercise, onClick = { viewModel.add(exercise, onPicked) })
                        HorizontalDivider(
                            color = MaterialTheme.colorScheme.outlineVariant,
                            modifier = Modifier.padding(horizontal = 20.dp),
                        )
                    }
                }
        }
    }
}

@Composable
private fun ExerciseRow(exercise: Exercise, onClick: () -> Unit) {
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .clickable(role = Role.Button, onClickLabel = "Add to Workout", onClick = onClick)
                .heightIn(min = 64.dp)
                .padding(horizontal = 20.dp, vertical = 10.dp),
    ) {
        Text(text = exercise.name, style = MaterialTheme.typography.titleMedium)
        Text(
            text =
                listOfNotNull(exercise.category, exercise.muscleGroup, exercise.equipment)
                    .joinToString(separator = " · "),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
