package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.SlabKey
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.formatKilograms
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import java.time.LocalDate
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

private val SET_NUMBER_WIDTH = 30.dp

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
fun WorkoutDetailRoute(workoutId: Long, onEdit: () -> Unit, onOpenActiveWorkout: () -> Unit) {
    val viewModel =
        appViewModel { WorkoutDetailViewModel(it.fitness, it.activeWorkout, it.syncStatus, it.saveScope, workoutId) }
    val lookup by viewModel.lookup.collectAsStateWithLifecycle()
    val sync by viewModel.sync.collectAsStateWithLifecycle()
    val active by viewModel.activeWorkout.collectAsStateWithLifecycle()
    when (val current = lookup) {
        WorkoutLookup.Loading -> Unit
        is WorkoutLookup.Loaded ->
            when (val workout = current.workout) {
                null -> EmptyState(title = "Workout not found", message = "This Workout isn't saved on this device.", glyph = Glyph.Rewind)
                else ->
                    WorkoutDetail(workout, sync, onSyncNow = viewModel::syncNow) {
                        Row(
                            Modifier.padding(horizontal = ScreenMargin),
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            RepeatAction(
                                active,
                                onRepeat = { viewModel.repeat(onOpenActiveWorkout) },
                                onResume = onOpenActiveWorkout,
                                modifier = Modifier.weight(1f),
                            )
                            GlyphKey(Glyph.Pencil, "Edit Workout", onEdit, tone = KeyTone.Raised, size = 72.dp)
                        }
                    }
            }
    }
}

@Composable
private fun WorkoutDetail(workout: Workout, sync: SyncStatus?, onSyncNow: () -> Unit, actions: @Composable () -> Unit) {
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        item {
            Row(
                Modifier.fillMaxWidth().padding(start = ScreenMargin + 4.dp, end = ScreenMargin, top = 16.dp, bottom = 6.dp),
                verticalAlignment = Alignment.Top,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(formatDay(workout.date), style = Type.Title, color = Lit.Ink)
                    workout.name?.let { Text(it, style = Type.Name, color = Lit.Lamp, modifier = Modifier.padding(top = 2.dp)) }
                }
                sync?.let { SyncStatusLine(it, onSyncNow = onSyncNow, modifier = Modifier.weight(0.5f, fill = false)) }
            }
            workout.notes?.let {
                Text(
                    text = it,
                    style = Type.Body,
                    color = Lit.Muted,
                    modifier = Modifier.padding(horizontal = ScreenMargin + 4.dp, vertical = 6.dp),
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
private fun RepeatAction(active: ActiveLookup, onRepeat: () -> Unit, onResume: () -> Unit, modifier: Modifier) {
    when (active) {
        ActiveLookup.Loading -> Spacer(modifier)
        is ActiveLookup.Loaded ->
            if (active.workout == null) {
                SlabKey(Glyph.Repeat, "Repeat This Workout", onRepeat, modifier = modifier)
            } else {
                SlabKey(Glyph.Play, "Resume Workout", onResume, modifier = modifier, tone = KeyTone.Raised)
                Spoken("Finish or discard your Active Workout to repeat this one.")
            }
    }
}

@Composable
private fun WorkoutEntryCard(entry: WorkoutEntry) {
    Slab(Modifier.fillMaxWidth().padding(horizontal = ScreenMargin), arrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            GlyphIcon(categoryGlyph(entry.exerciseCategory), tint = Lit.Lamp, size = 22.dp)
            Text(text = entry.exerciseName, style = Type.Name, color = Lit.Ink)
        }
        entry.notes?.let { Text(text = it, style = Type.Body, color = Lit.Muted) }
        entry.sets.forEach { SetRow(it) }
    }
}

@Composable
private fun SetRow(set: WorkoutSet) {
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = set.setNumber.toString(),
                style = Type.Small,
                color = Lit.Muted,
                modifier = Modifier.width(SET_NUMBER_WIDTH),
            )
            Text(shortPerformance(set), style = Type.Figure, color = Lit.Ink, modifier = Modifier.weight(1f))
            Spoken(describePerformance(set))
            set.rpe?.let {
                Spoken("RPE $it")
                GlyphIcon(Glyph.Gauge, tint = Lit.Muted, size = 16.dp)
                Spacer(Modifier.width(6.dp))
                Text(it.toString(), style = Type.Name, color = Lit.Muted)
            }
        }
        set.notes?.let {
            Text(text = it, style = Type.Body, color = Lit.Muted, modifier = Modifier.padding(start = SET_NUMBER_WIDTH))
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
