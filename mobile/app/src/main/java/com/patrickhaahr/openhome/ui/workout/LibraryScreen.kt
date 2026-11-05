package com.patrickhaahr.openhome.ui.workout

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.Exercise
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.storage.SyncProblem
import com.patrickhaahr.openhome.sync.FitnessSyncStatus
import com.patrickhaahr.openhome.sync.SyncStatus
import com.patrickhaahr.openhome.ui.appViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn

private class LibraryRowModel(val exercise: Exercise, val sync: SyncStatus, val refusal: SyncProblem?)

private class LibraryViewModel(fitness: FitnessRepository, syncStatus: FitnessSyncStatus) : ViewModel() {
    val rows: StateFlow<LibraryLookup<LibraryRowModel>> =
        combine<_, _, LibraryLookup<LibraryRowModel>>(fitness.observeExercises(), syncStatus.observe()) { exercises, snapshot ->
            LibraryLookup.Loaded(
                exercises.map { LibraryRowModel(it, snapshot.exercise(it.id), snapshot.exerciseRefusal(it.id)) },
            )
        }
            .catch { emit(LibraryLookup.Unreadable) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), LibraryLookup.Loading)
}

@Composable
fun LibraryRoute(onCreate: (String) -> Unit, onFix: (Long) -> Unit) {
    val viewModel = appViewModel { LibraryViewModel(it.fitness, it.syncStatus) }
    val rows by viewModel.rows.collectAsStateWithLifecycle()
    ExerciseBrowser(
        title = "Library",
        lookup = rows,
        exerciseOf = { it.exercise },
        onCreate = onCreate,
        row = { row ->
            val refusal = row.refusal
            ExerciseRow(
                exercise = row.exercise,
                onClick = refusal?.let { { onFix(row.exercise.id) } },
                onClickLabel = "Fix Exercise",
            ) {
                when {
                    refusal != null ->
                        Text(
                            text = exerciseRefusalMessage(refusal),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.error,
                        )
                    // Quiet by default: a downloaded or synced Exercise says nothing here.
                    row.sync != SyncStatus.Synced -> SyncStatusLine(row.sync)
                }
            }
        },
    )
}
