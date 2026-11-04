package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.fitness.WorkoutSummary
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.pluralize
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn

class HistoryViewModel(fitness: FitnessRepository) : ViewModel() {
    val workouts: StateFlow<List<WorkoutSummary>?> =
        fitness.observeHistory().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)
}

@Composable
fun HistoryRoute(onOpenWorkout: (Long) -> Unit) {
    val viewModel = appViewModel { HistoryViewModel(it.fitness) }
    val workouts by viewModel.workouts.collectAsStateWithLifecycle()
    val listState = rememberLazyListState()

    Column(Modifier.fillMaxSize()) {
        ScreenHeader(
            title = "History",
            subtitle = workouts?.let { "${pluralize(it.size, "Workout")} on this device" },
        )
        when (val rows = workouts) {
            null -> Unit
            emptyList<WorkoutSummary>() ->
                EmptyState(title = "No Workouts yet", message = "Workouts you log will appear here.")
            else ->
                LazyColumn(state = listState, contentPadding = PaddingValues(bottom = 16.dp)) {
                    items(rows, key = { it.id }) { workout ->
                        HistoryRow(workout, onClick = { onOpenWorkout(workout.id) })
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
private fun HistoryRow(workout: WorkoutSummary, onClick: () -> Unit) {
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .clickable(role = Role.Button, onClickLabel = "Open Workout", onClick = onClick)
                .heightIn(min = 72.dp)
                .padding(horizontal = 20.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Text(text = formatDay(workout.date), style = MaterialTheme.typography.titleMedium)
        workout.name?.let { Text(text = it, style = MaterialTheme.typography.bodyLarge) }
        Text(
            text =
                listOfNotNull(workout.exerciseNames, pluralize(workout.setCount, "Set"))
                    .joinToString(separator = " · "),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}
