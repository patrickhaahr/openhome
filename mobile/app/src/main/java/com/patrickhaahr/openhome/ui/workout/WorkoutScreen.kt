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

    fun startEmpty(onStarted: () -> Unit) {
        saveScope.launch {
            activeWorkouts.startEmpty(LocalDate.now().toString())
            onStarted()
        }
    }

    fun syncNow() {
        saveScope.launch { syncStatus.syncNow() }
    }
}

@Composable
fun WorkoutRoute(onOpenHistory: () -> Unit, onOpenProfile: () -> Unit, onOpenActiveWorkout: () -> Unit) {
    val viewModel =
        appViewModel { WorkoutViewModel(it.fitness, it.activeWorkout, it.syncStatus, it.saveScope) }
    val overview by viewModel.overview.collectAsStateWithLifecycle()
    val active by viewModel.activeWorkout.collectAsStateWithLifecycle()
    val sync by viewModel.sync.collectAsStateWithLifecycle()
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
                        null -> StartEmptyAction(onClick = { viewModel.startEmpty(onOpenActiveWorkout) })
                        else -> ResumeAction(workout, onClick = onOpenActiveWorkout)
                    }
            }
            overview?.let { WorkoutOverview(it, onOpenHistory) }
            sync?.let {
                SyncStatusLine(it, onSyncNow = viewModel::syncNow, modifier = Modifier.padding(horizontal = 20.dp))
            }
        }
    }
}

@Composable
private fun StartEmptyAction(onClick: () -> Unit) {
    Button(onClick = onClick, modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp)) {
        Text(text = "Start Empty", style = MaterialTheme.typography.titleMedium)
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
private fun WorkoutOverview(overview: FitnessOverview, onOpenHistory: () -> Unit) {
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
        Surface(
            color = MaterialTheme.colorScheme.surfaceContainer,
            shape = MaterialTheme.shapes.large,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Row(
                modifier =
                    Modifier
                        .clickable(role = Role.Button, onClick = onOpenHistory)
                        .heightIn(min = 72.dp)
                        .padding(horizontal = 20.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(text = "History", style = MaterialTheme.typography.titleMedium)
                    Text(
                        text = "${pluralize(overview.workoutCount, "Workout")} on this device",
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
        Text(
            text = "${pluralize(overview.exerciseCount, "Exercise")} in your Library",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp),
        )
    }
}
