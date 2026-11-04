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
import com.patrickhaahr.openhome.fitness.FitnessOverview
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.pluralize
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn

class WorkoutViewModel(fitness: FitnessRepository) : ViewModel() {
    val overview: StateFlow<FitnessOverview?> =
        fitness.observeOverview().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)
}

@Composable
fun WorkoutRoute(onOpenHistory: () -> Unit, onOpenProfile: () -> Unit) {
    val viewModel = appViewModel { WorkoutViewModel(it.fitness) }
    val overview by viewModel.overview.collectAsStateWithLifecycle()
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        ScreenHeader(title = "Workout", onOpenProfile = onOpenProfile)
        overview?.let { WorkoutOverview(it, onOpenHistory) }
    }
}

@Composable
private fun WorkoutOverview(overview: FitnessOverview, onOpenHistory: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
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
