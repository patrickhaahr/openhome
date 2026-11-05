package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.R
import com.patrickhaahr.openhome.fitness.ActiveWorkout
import com.patrickhaahr.openhome.fitness.ActiveWorkoutRepository
import com.patrickhaahr.openhome.fitness.FitnessOverview
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.fitness.RefusedExercise
import com.patrickhaahr.openhome.fitness.WorkoutSummary
import com.patrickhaahr.openhome.sync.FitnessSyncStatus
import com.patrickhaahr.openhome.sync.SyncSnapshot
import com.patrickhaahr.openhome.sync.SyncStatus
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.pluralize
import java.time.LocalDate
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class WorkoutViewModel(
    fitness: FitnessRepository,
    private val activeWorkouts: ActiveWorkoutRepository,
    private val syncStatus: FitnessSyncStatus,
    private val saveScope: CoroutineScope,
) : ViewModel() {
    val overview: StateFlow<FitnessOverview?> =
        fitness.observeOverview().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    val activeWorkout: StateFlow<ActiveLookup> =
        activeWorkouts.observe()
            .map<ActiveWorkout?, ActiveLookup> { ActiveLookup.Loaded(it) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ActiveLookup.Loading)

    val sync: StateFlow<SyncStatus?> =
        syncStatus.observe()
            .map<SyncSnapshot, SyncStatus?> { it.overall }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    /** What only the user can settle; everything queued behind these waits for them. */
    val refusedExercises: StateFlow<List<RefusedExercise>> =
        combine(fitness.observeExercises(), syncStatus.observe()) { exercises, snapshot ->
            exercises.mapNotNull { exercise ->
                snapshot.exerciseRefusal(exercise.id)?.let { RefusedExercise(exercise, it) }
            }
        }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun startEmpty(onStarted: () -> Unit) {
        saveScope.launch {
            activeWorkouts.startEmpty(LocalDate.now().toString())
            onStarted()
        }
    }

    fun repeat(workout: WorkoutSummary, onStarted: () -> Unit) {
        saveScope.launch {
            if (activeWorkouts.repeat(workout.id, LocalDate.now().toString())) onStarted()
        }
    }

    fun syncNow() {
        saveScope.launch { syncStatus.syncNow() }
    }
}

@Composable
fun WorkoutRoute(
    onOpenHistory: () -> Unit,
    onOpenLibrary: () -> Unit,
    onFixExercise: (Long) -> Unit,
    onOpenProfile: () -> Unit,
    onOpenActiveWorkout: () -> Unit,
) {
    val viewModel =
        appViewModel { WorkoutViewModel(it.fitness, it.activeWorkout, it.syncStatus, it.saveScope) }
    val overview by viewModel.overview.collectAsStateWithLifecycle()
    val active by viewModel.activeWorkout.collectAsStateWithLifecycle()
    val sync by viewModel.sync.collectAsStateWithLifecycle()
    val refusedExercises by viewModel.refusedExercises.collectAsStateWithLifecycle()
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        ScreenHeader(title = "Workout", onOpenProfile = onOpenProfile)
        Column(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            when (val current = active) {
                ActiveLookup.Loading -> Unit
                is ActiveLookup.Loaded ->
                    when (val workout = current.workout) {
                        // Waits for the overview, so Start Empty is never shown as primary by mistake.
                        null ->
                            overview?.let {
                                StartActions(
                                    latest = it.latestWorkout,
                                    onRepeat = { latest -> viewModel.repeat(latest, onOpenActiveWorkout) },
                                    onStartEmpty = { viewModel.startEmpty(onOpenActiveWorkout) },
                                )
                            }
                        else -> ResumeAction(workout, onClick = onOpenActiveWorkout)
                    }
            }
            refusedExercises.forEach { row ->
                RefusedExerciseCard(row, onClick = { onFixExercise(row.exercise.id) })
            }
            overview?.let { WorkoutOverview(it, onOpenHistory, onOpenLibrary) }
            sync?.let {
                SyncStatusLine(it, onSyncNow = viewModel::syncNow, modifier = Modifier.padding(horizontal = 20.dp))
            }
        }
    }
}

/** Repeating is the usual session, so it leads; with no History, starting empty is all there is. */
@Composable
private fun StartActions(latest: WorkoutSummary?, onRepeat: (WorkoutSummary) -> Unit, onStartEmpty: () -> Unit) {
    if (latest == null) {
        Button(onClick = onStartEmpty, modifier = Modifier.fillMaxWidth().heightIn(min = 72.dp)) {
            Text(text = "Start Empty", style = MaterialTheme.typography.titleMedium)
        }
    } else {
        Button(onClick = { onRepeat(latest) }, modifier = Modifier.fillMaxWidth().heightIn(min = 72.dp)) {
            Text(text = "Repeat Last Workout", style = MaterialTheme.typography.titleMedium)
        }
        OutlinedButton(onClick = onStartEmpty, modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp)) {
            Text(text = "Start Empty", style = MaterialTheme.typography.titleMedium)
        }
    }
}

@Composable
private fun ResumeAction(workout: ActiveWorkout, onClick: () -> Unit) {
    val confirmed = workout.entries.sumOf { entry -> entry.sets.count { it.completed } }
    Button(onClick = onClick, modifier = Modifier.fillMaxWidth().heightIn(min = 72.dp)) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(text = "Resume Workout", style = MaterialTheme.typography.titleMedium)
            Text(
                text = "${formatDay(workout.date)} · ${pluralize(confirmed, "Set")} confirmed",
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}

@Composable
private fun WorkoutOverview(overview: FitnessOverview, onOpenHistory: () -> Unit, onOpenLibrary: () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Surface(
            color = MaterialTheme.colorScheme.surfaceContainer,
            shape = MaterialTheme.shapes.large,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    text = "Last Workout",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                val latest = overview.latestWorkout
                if (latest == null) {
                    Text(text = "No Workouts yet", style = MaterialTheme.typography.headlineSmall)
                    Text(
                        text = "Workouts you log will appear here.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                } else {
                    Text(text = formatDay(latest.date), style = MaterialTheme.typography.headlineSmall)
                    latest.name?.let { Text(text = it, style = MaterialTheme.typography.titleSmall) }
                    latest.exerciseNames?.let {
                        Text(
                            text = it,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
        OverviewLink(
            title = "History",
            subtitle = "${pluralize(overview.workoutCount, "Workout")} on this device",
            onClick = onOpenHistory,
        )
        OverviewLink(
            title = "Library",
            subtitle = "${pluralize(overview.exerciseCount, "Exercise")} in your Library",
            onClick = onOpenLibrary,
        )
    }
}

@Composable
private fun OverviewLink(title: String, subtitle: String, onClick: () -> Unit) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainer,
        shape = MaterialTheme.shapes.large,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier =
                Modifier
                    .clickable(role = Role.Button, onClick = onClick)
                    .heightIn(min = 72.dp)
                    .padding(horizontal = 20.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(text = title, style = MaterialTheme.typography.titleMedium)
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Icon(
                painterResource(R.drawable.ic_chevron_right),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun RefusedExerciseCard(row: RefusedExercise, onClick: () -> Unit) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainer,
        shape = MaterialTheme.shapes.large,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier =
                Modifier
                    .clickable(role = Role.Button, onClickLabel = "Fix Exercise", onClick = onClick)
                    .heightIn(min = 72.dp)
                    .padding(horizontal = 20.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(text = row.exercise.name, style = MaterialTheme.typography.titleMedium)
                Text(
                    text = exerciseRefusalMessage(row.problem),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                )
            }
            Icon(
                painterResource(R.drawable.ic_chevron_right),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
