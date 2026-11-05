package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.ActiveWorkout
import com.patrickhaahr.openhome.fitness.ActiveWorkoutRepository
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.fitness.Workout
import com.patrickhaahr.openhome.fitness.WorkoutEntry
import com.patrickhaahr.openhome.fitness.WorkoutSet
import com.patrickhaahr.openhome.sync.FitnessSyncStatus
import com.patrickhaahr.openhome.sync.SyncSnapshot
import com.patrickhaahr.openhome.sync.SyncStatus
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.formatKilograms
import java.time.LocalDate
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

private val SET_NUMBER_WIDTH = 28.dp

private sealed interface WorkoutLookup {
    data object Loading : WorkoutLookup

    data class Loaded(val workout: Workout?) : WorkoutLookup
}

private class WorkoutDetailViewModel(
    fitness: FitnessRepository,
    private val activeWorkouts: ActiveWorkoutRepository,
    private val syncStatus: FitnessSyncStatus,
    private val saveScope: CoroutineScope,
    private val workoutId: Long,
) : ViewModel() {
    val lookup: StateFlow<WorkoutLookup> =
        fitness.observeWorkout(workoutId)
            .map<Workout?, WorkoutLookup> { WorkoutLookup.Loaded(it) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), WorkoutLookup.Loading)

    val sync: StateFlow<SyncStatus?> =
        syncStatus.observe()
            .map<SyncSnapshot, SyncStatus?> { it.workout(workoutId) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    val activeWorkout: StateFlow<ActiveLookup> =
        activeWorkouts.observe()
            .map<ActiveWorkout?, ActiveLookup> { ActiveLookup.Loaded(it) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ActiveLookup.Loading)

    fun syncNow() {
        saveScope.launch { syncStatus.syncNow() }
    }

    fun repeat(onStarted: () -> Unit) {
        saveScope.launch {
            if (activeWorkouts.repeat(workoutId, LocalDate.now().toString())) onStarted()
        }
    }
}

@Composable
fun WorkoutDetailRoute(workoutId: Long, onOpenActiveWorkout: () -> Unit) {
    val viewModel =
        appViewModel { WorkoutDetailViewModel(it.fitness, it.activeWorkout, it.syncStatus, it.saveScope, workoutId) }
    val lookup by viewModel.lookup.collectAsStateWithLifecycle()
    val sync by viewModel.sync.collectAsStateWithLifecycle()
    val active by viewModel.activeWorkout.collectAsStateWithLifecycle()
    when (val current = lookup) {
        WorkoutLookup.Loading -> Unit
        is WorkoutLookup.Loaded ->
            when (val workout = current.workout) {
                null -> EmptyState(title = "Workout not found", message = "This Workout isn't saved on this device.")
                else ->
                    WorkoutDetail(workout, sync, onSyncNow = viewModel::syncNow) {
                        RepeatAction(
                            active,
                            onRepeat = { viewModel.repeat(onOpenActiveWorkout) },
                            onResume = onOpenActiveWorkout,
                        )
                    }
            }
    }
}

@Composable
private fun WorkoutDetail(workout: Workout, sync: SyncStatus?, onSyncNow: () -> Unit, actions: @Composable () -> Unit) {
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            ScreenHeader(title = formatDay(workout.date), subtitle = workout.name)
            sync?.let { SyncStatusLine(it, onSyncNow = onSyncNow, modifier = Modifier.padding(horizontal = 20.dp)) }
            workout.notes?.let {
                Text(
                    text = it,
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 20.dp),
                )
            }
        }
        item { actions() }
        if (workout.entries.isEmpty()) {
            item { EmptyState(title = "No Exercises", message = "This Workout has no logged Exercises.") }
        }
        items(workout.entries, key = { it.id }) { WorkoutEntryCard(it) }
    }
}

/** There is one Active Workout at a time, so an unfinished one is resumed rather than replaced. */
@Composable
private fun RepeatAction(active: ActiveLookup, onRepeat: () -> Unit, onResume: () -> Unit) {
    val modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp).heightIn(min = 56.dp)
    when (active) {
        ActiveLookup.Loading -> Unit
        is ActiveLookup.Loaded ->
            if (active.workout == null) {
                Button(onClick = onRepeat, modifier = modifier) {
                    Text(text = "Repeat This Workout", style = MaterialTheme.typography.titleMedium)
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = onResume, modifier = modifier) { Text("Resume Workout") }
                    Text(
                        text = "Finish or discard your Active Workout to repeat this one.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 20.dp),
                    )
                }
            }
    }
}

@Composable
private fun WorkoutEntryCard(entry: WorkoutEntry) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainer,
        shape = MaterialTheme.shapes.large,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(text = entry.exerciseName, style = MaterialTheme.typography.titleMedium)
            entry.notes?.let {
                Text(
                    text = it,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            entry.sets.forEach { SetRow(it) }
        }
    }
}

@Composable
private fun SetRow(set: WorkoutSet) {
    Column(Modifier.semantics(mergeDescendants = true) {}) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = set.setNumber.toString(),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.width(SET_NUMBER_WIDTH),
            )
            Text(
                text = describePerformance(set),
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.weight(1f),
            )
            set.rpe?.let {
                Text(
                    text = "RPE $it",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        set.notes?.let {
            Text(
                text = it,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = SET_NUMBER_WIDTH),
            )
        }
    }
}

/** Added Weight reads as "+40 kg" and null as "Bodyweight"; a Timed Hold shows its duration, not reps. */
fun describePerformance(set: WorkoutSet): String {
    val amount =
        listOfNotNull(
            set.reps?.let { if (it == 1L) "1 rep" else "$it reps" },
            set.durationSeconds?.let { "$it s" },
        )
    val load = set.addedWeightKg?.let { "+${formatKilograms(it)}" } ?: "Bodyweight"
    return (amount + load).joinToString(separator = " · ")
}
