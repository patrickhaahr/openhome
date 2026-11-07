package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.fitness.WorkoutSummary
import com.patrickhaahr.openhome.sync.FitnessSyncStatus
import com.patrickhaahr.openhome.sync.SyncStatus
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.SlabShape
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.dayOfMonth
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.formatMonth
import com.patrickhaahr.openhome.ui.pluralize
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import com.patrickhaahr.openhome.ui.yearOf
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn

private val DAY_COLUMN = 64.dp

private class HistoryRowModel(val workout: WorkoutSummary, val sync: SyncStatus)

private class HistoryViewModel(fitness: FitnessRepository, syncStatus: FitnessSyncStatus) : ViewModel() {
    val workouts: StateFlow<List<HistoryRowModel>?> =
        combine(fitness.observeHistory(), syncStatus.observe()) { workouts, snapshot ->
            workouts.map { HistoryRowModel(it, snapshot.workout(it.id)) }
        }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)
}

/** A training log, as it would be kept on paper: the day large in the margin, the year where it turns. */
@Composable
fun HistoryRoute(onOpenWorkout: (Long) -> Unit) {
    val viewModel = appViewModel { HistoryViewModel(it.fitness, it.syncStatus) }
    val workouts by viewModel.workouts.collectAsStateWithLifecycle()
    val listState = rememberLazyListState()

    Column(Modifier.fillMaxSize()) {
        ScreenHeader(
            title = "History",
            glyph = Glyph.Rewind,
            figure = workouts?.size?.toString(),
            subtitle = workouts?.let { "${pluralize(it.size, "Workout")} on this device" },
        )
        when (val rows = workouts) {
            null -> Unit
            emptyList<HistoryRowModel>() ->
                EmptyState(title = "No Workouts yet", message = "Workouts you log will appear here.", glyph = Glyph.Rewind)
            else ->
                LazyColumn(
                    state = listState,
                    contentPadding = PaddingValues(start = ScreenMargin, end = ScreenMargin, bottom = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    itemsIndexed(rows, key = { _, row -> row.workout.id }) { index, row ->
                        val year = yearOf(row.workout.date)
                        if (index > 0 && year != yearOf(rows[index - 1].workout.date) && year != null) {
                            Text(
                                year.toString(),
                                style = Type.Title,
                                color = Lit.Lamp,
                                modifier = Modifier.padding(start = 4.dp, top = 28.dp, bottom = 8.dp),
                            )
                        }
                        HistoryRow(row, onClick = { onOpenWorkout(row.workout.id) })
                    }
                }
        }
    }
}

@Composable
private fun HistoryRow(row: HistoryRowModel, onClick: () -> Unit) {
    val workout = row.workout
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = 76.dp)
                .key("Open ${formatDay(workout.date)}", onClick, shape = SlabShape, depth = 0.98f)
                .padding(horizontal = 4.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.width(DAY_COLUMN)) {
            Text(dayOfMonth(workout.date), style = Type.Title, color = Lit.Ink)
            Text(formatMonth(workout.date), style = Type.Small, color = Lit.Muted)
        }
        Column(Modifier.weight(1f).padding(end = 12.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Spoken(formatDay(workout.date))
            workout.name?.let { Text(it, style = Type.Name, color = Lit.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis) }
            workout.exerciseNames?.let {
                Text(it, style = Type.Body, color = Lit.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            // Quiet by default: a downloaded or synced Workout says nothing here.
            if (row.sync != SyncStatus.Synced) SyncStatusLine(row.sync)
        }
        Spoken(listOfNotNull(workout.exerciseNames, pluralize(workout.setCount, "Set")).joinToString(separator = " · "))
        Text(workout.setCount.toString(), style = Type.Figure, color = Lit.Ink)
        Spacer(Modifier.width(6.dp))
        GlyphIcon(Glyph.Check, tint = Lit.Lamp, size = 18.dp)
    }
}
