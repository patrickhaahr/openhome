package com.patrickhaahr.openhome.ui.workout

import android.database.SQLException
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SelectableDates
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.ActiveEntry
import com.patrickhaahr.openhome.fitness.ActiveSet
import com.patrickhaahr.openhome.fitness.ActiveWorkout
import com.patrickhaahr.openhome.fitness.ActiveWorkoutRepository
import com.patrickhaahr.openhome.fitness.FinishOutcome
import com.patrickhaahr.openhome.fitness.SetCheck
import com.patrickhaahr.openhome.fitness.SetField
import com.patrickhaahr.openhome.storage.SetMeasure
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.pluralize
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

private val SET_NUMBER_WIDTH = 28.dp
private val CONFIRM_WIDTH = 48.dp

sealed interface ActiveLookup {
    data object Loading : ActiveLookup

    data class Loaded(val workout: ActiveWorkout?) : ActiveLookup
}

private enum class SaveState {
    SAVING,
    SAVED,
    FAILED,
}

private class ActiveWorkoutViewModel(
    private val repository: ActiveWorkoutRepository,
    private val saveScope: CoroutineScope,
) : ViewModel() {
    private val writesInProgress = MutableStateFlow(0)
    private val lastWriteFailed = MutableStateFlow(false)
    private val refusedConfirmations = MutableStateFlow(emptySet<Long>())

    val workout: StateFlow<ActiveLookup> =
        repository.observe()
            .map<ActiveWorkout?, ActiveLookup> { ActiveLookup.Loaded(it) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ActiveLookup.Loading)

    /** Says saved only when every change made so far has committed. */
    val saveState: StateFlow<SaveState> =
        combine(writesInProgress, lastWriteFailed) { writing, failed ->
            when {
                writing > 0 -> SaveState.SAVING
                failed -> SaveState.FAILED
                else -> SaveState.SAVED
            }
        }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), SaveState.SAVED)

    /** Rows whose confirmation was refused, so only they explain what is missing. */
    val refusedSets: StateFlow<Set<Long>> = refusedConfirmations.asStateFlow()

    fun setDate(date: LocalDate) = save { repository.setDate(date.toString()) }

    fun setName(name: String) = save { repository.setName(name) }

    fun setNotes(notes: String) = save { repository.setNotes(notes) }

    fun addSet(entryId: Long) = save { repository.addSet(entryId) }

    fun removeEntry(entryId: Long) = save { repository.removeEntry(entryId) }

    fun setMeasure(entryId: Long, measure: SetMeasure) = save { repository.setMeasure(entryId, measure) }

    fun editSet(setId: Long, field: SetField, text: String) = save {
        repository.editSet(setId, field, text)
        refusedConfirmations.update { it - setId }
    }

    fun setCompleted(setId: Long, completed: Boolean) = save {
        if (!repository.setCompleted(setId, completed)) refusedConfirmations.update { it + setId }
    }

    fun finish(omitUnchecked: Boolean, onFinished: () -> Unit) = save {
        if (repository.finish(omitUnchecked) is FinishOutcome.Finished) onFinished()
    }

    fun discard(onDiscarded: () -> Unit) = save {
        repository.discard()
        onDiscarded()
    }

    /** In the app's scope, so leaving the screen right after a change cannot cancel its save. */
    private fun save(change: suspend () -> Unit) {
        writesInProgress.update { it + 1 }
        saveScope.launch {
            try {
                change()
                lastWriteFailed.value = false
            } catch (_: SQLException) {
                lastWriteFailed.value = true
            } finally {
                writesInProgress.update { it - 1 }
            }
        }
    }
}

/** Leaving this destination, by back or by tab, keeps the Active Workout; it is all in storage. */
@Composable
fun ActiveWorkoutRoute(onAddExercise: () -> Unit, onFinished: () -> Unit, onDiscarded: () -> Unit) {
    val viewModel = appViewModel { ActiveWorkoutViewModel(it.activeWorkout, it.saveScope) }
    val lookup by viewModel.workout.collectAsStateWithLifecycle()
    val saveState by viewModel.saveState.collectAsStateWithLifecycle()
    val refusedSets by viewModel.refusedSets.collectAsStateWithLifecycle()

    when (val current = lookup) {
        ActiveLookup.Loading -> Unit
        is ActiveLookup.Loaded ->
            when (val workout = current.workout) {
                null -> EmptyState(title = "No Active Workout", message = "Start one from the Workout tab.")
                else ->
                    ActiveWorkoutContent(
                        workout = workout,
                        saveState = saveState,
                        refusedSets = refusedSets,
                        viewModel = viewModel,
                        onAddExercise = onAddExercise,
                        onFinished = onFinished,
                        onDiscarded = onDiscarded,
                    )
            }
    }
}

@Composable
private fun ActiveWorkoutContent(
    workout: ActiveWorkout,
    saveState: SaveState,
    refusedSets: Set<Long>,
    viewModel: ActiveWorkoutViewModel,
    onAddExercise: () -> Unit,
    onFinished: () -> Unit,
    onDiscarded: () -> Unit,
) {
    var choosingDate by rememberSaveable { mutableStateOf(false) }
    var reviewingFinish by rememberSaveable { mutableStateOf(false) }
    var confirmingDiscard by rememberSaveable { mutableStateOf(false) }

    LazyColumn(
        modifier = Modifier.fillMaxSize().imePadding(),
        contentPadding = PaddingValues(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            ScreenHeader(
                title = "Active Workout",
                subtitle =
                    when (saveState) {
                        SaveState.SAVING -> "Saving on device"
                        SaveState.SAVED -> "Saved on device"
                        SaveState.FAILED -> "Couldn't save on device. Your last change may be lost."
                    },
            )
            SessionFields(
                workout,
                viewModel,
                onChooseDate = { choosingDate = true },
                modifier = Modifier.padding(horizontal = 16.dp),
            )
        }
        items(workout.entries, key = { it.id }) { entry ->
            EntryCard(entry, refusedSets, viewModel)
        }
        item {
            Column(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(onClick = onAddExercise, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) {
                    Text("Add Exercise")
                }
                Button(
                    onClick = {
                        if (workout.hasOnlyConfirmedSets()) {
                            viewModel.finish(omitUnchecked = false, onFinished)
                        } else {
                            reviewingFinish = true
                        }
                    },
                    modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp),
                ) {
                    Text(text = "Finish Workout", style = MaterialTheme.typography.titleMedium)
                }
                TextButton(onClick = { confirmingDiscard = true }, modifier = Modifier.fillMaxWidth()) {
                    Text(text = "Discard Workout", color = MaterialTheme.colorScheme.error)
                }
            }
        }
    }

    if (choosingDate) {
        WorkoutDatePicker(
            date = workout.date,
            onChosen = viewModel::setDate,
            onDismiss = { choosingDate = false },
        )
    }
    if (reviewingFinish) {
        FinishReview(
            workout,
            onReturn = { reviewingFinish = false },
            onOmitAndFinish = {
                reviewingFinish = false
                viewModel.finish(omitUnchecked = true, onFinished)
            },
        )
    }
    if (confirmingDiscard) {
        AlertDialog(
            onDismissRequest = { confirmingDiscard = false },
            title = { Text("Discard this Workout?") },
            text = { Text("Its Exercises and Sets will be deleted from this device.") },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmingDiscard = false
                        viewModel.discard(onDiscarded)
                    },
                ) {
                    Text("Discard")
                }
            },
            dismissButton = { TextButton(onClick = { confirmingDiscard = false }) { Text("Keep logging") } },
        )
    }
}

private fun ActiveWorkout.hasOnlyConfirmedSets(): Boolean =
    entries.isNotEmpty() && entries.all { entry -> entry.sets.all { it.completed } }

@Composable
private fun SessionFields(
    workout: ActiveWorkout,
    viewModel: ActiveWorkoutViewModel,
    onChooseDate: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var name by rememberSaveable { mutableStateOf(workout.name) }
    var notes by rememberSaveable { mutableStateOf(workout.notes) }
    Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        OutlinedButton(onClick = onChooseDate, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) {
            Text(formatDay(workout.date))
        }
        OutlinedTextField(
            value = name,
            onValueChange = {
                name = it
                viewModel.setName(it)
            },
            label = { Text("Name (optional)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = notes,
            onValueChange = {
                notes = it
                viewModel.setNotes(it)
            },
            label = { Text("Notes (optional)") },
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

@Composable
private fun EntryCard(entry: ActiveEntry, refusedSets: Set<Long>, viewModel: ActiveWorkoutViewModel) {
    val measure = entry.sets.first().draft.measure
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainer,
        shape = MaterialTheme.shapes.large,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
    ) {
        Column(Modifier.padding(start = 16.dp, end = 8.dp, top = 12.dp, bottom = 4.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(text = entry.exerciseName, style = MaterialTheme.typography.titleMedium)
                    Text(
                        text = entry.exerciseCategory,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                TextButton(
                    onClick = { viewModel.removeEntry(entry.id) },
                    modifier = Modifier.semantics { contentDescription = "Remove ${entry.exerciseName}" },
                ) {
                    Text("Remove")
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                MeasureChip(entry, SetMeasure.REPS, "Reps", measure, viewModel)
                MeasureChip(entry, SetMeasure.HOLD, "Hold", measure, viewModel)
            }
            SetColumnLabels(measure)
            entry.sets.forEach { set ->
                SetRow(entry.exerciseName, set, showMissingValue = set.id in refusedSets, viewModel)
            }
            TextButton(
                onClick = { viewModel.addSet(entry.id) },
                modifier = Modifier.semantics { contentDescription = "Add set to ${entry.exerciseName}" },
            ) {
                Text("Add set")
            }
        }
    }
}

@Composable
private fun MeasureChip(
    entry: ActiveEntry,
    measure: SetMeasure,
    label: String,
    selected: SetMeasure,
    viewModel: ActiveWorkoutViewModel,
) {
    FilterChip(
        selected = selected == measure,
        onClick = { viewModel.setMeasure(entry.id, measure) },
        label = { Text(label) },
        modifier = Modifier.semantics { contentDescription = "Log ${entry.exerciseName} as $label" },
    )
}

@Composable
private fun SetColumnLabels(measure: SetMeasure) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        ColumnLabel("Set", Modifier.width(SET_NUMBER_WIDTH))
        ColumnLabel(if (measure == SetMeasure.REPS) "Reps" else "Seconds", Modifier.weight(1f))
        ColumnLabel("Added kg", Modifier.weight(1f))
        ColumnLabel("RPE", Modifier.weight(1f))
        Box(Modifier.width(CONFIRM_WIDTH))
    }
}

@Composable
private fun ColumnLabel(text: String, modifier: Modifier) {
    Text(
        text = text,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier,
    )
}

@Composable
private fun SetRow(exerciseName: String, set: ActiveSet, showMissingValue: Boolean, viewModel: ActiveWorkoutViewModel) {
    val problem = set.draft.check() as? SetCheck.Problem
    val label = "$exerciseName set ${set.number}"
    Column {
        Row(
            modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = set.number.toString(),
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.width(SET_NUMBER_WIDTH),
            )
            when (set.draft.measure) {
                SetMeasure.REPS ->
                    SetValueField(set, SetField.REPS, set.draft.reps, "$label reps", KeyboardType.Number, viewModel)
                SetMeasure.HOLD ->
                    SetValueField(
                        set,
                        SetField.DURATION,
                        set.draft.durationSeconds,
                        "$label seconds",
                        KeyboardType.Number,
                        viewModel,
                    )
            }
            SetValueField(
                set,
                SetField.ADDED_WEIGHT,
                set.draft.addedWeightKg,
                "$label added kg",
                KeyboardType.Decimal,
                viewModel,
            )
            SetValueField(set, SetField.RPE, set.draft.rpe, "$label RPE", KeyboardType.Number, viewModel)
            Box(Modifier.width(CONFIRM_WIDTH), contentAlignment = Alignment.Center) {
                Checkbox(
                    checked = set.completed,
                    onCheckedChange = { viewModel.setCompleted(set.id, it) },
                    modifier = Modifier.semantics { contentDescription = "$label performed" },
                )
            }
        }
        if (problem != null && (!problem.isMissingValue || showMissingValue)) {
            Text(
                text = problem.message,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(start = SET_NUMBER_WIDTH + 8.dp, bottom = 4.dp),
            )
        }
    }
}

/** Keeps its own text, so a save finishing later can never move the cursor or undo typing. */
@Composable
private fun RowScope.SetValueField(
    set: ActiveSet,
    field: SetField,
    stored: String,
    description: String,
    keyboard: KeyboardType,
    viewModel: ActiveWorkoutViewModel,
) {
    var text by rememberSaveable(set.id, field) { mutableStateOf(stored) }
    BasicTextField(
        value = text,
        onValueChange = {
            text = it
            viewModel.editSet(set.id, field, it)
        },
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onSurface),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        keyboardOptions = KeyboardOptions(keyboardType = keyboard),
        modifier =
            Modifier
                .weight(1f)
                .heightIn(min = 48.dp)
                .background(
                    if (set.completed) {
                        MaterialTheme.colorScheme.primaryContainer
                    } else {
                        MaterialTheme.colorScheme.surfaceContainerHighest
                    },
                    MaterialTheme.shapes.small,
                ),
        decorationBox = { inner ->
            // Inside the decoration, the description labels the field the way a visible label would.
            Box(
                modifier =
                    Modifier
                        .heightIn(min = 48.dp)
                        .padding(horizontal = 10.dp)
                        .semantics { contentDescription = description },
                contentAlignment = Alignment.CenterStart,
            ) {
                inner()
            }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun WorkoutDatePicker(date: String, onChosen: (LocalDate) -> Unit, onDismiss: () -> Unit) {
    val today = remember { LocalDate.now() }
    val state =
        rememberDatePickerState(
            initialSelectedDateMillis = LocalDate.parse(date).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli(),
            selectableDates =
                object : SelectableDates {
                    override fun isSelectableDate(utcTimeMillis: Long) = utcTimeMillis.toUtcDay() <= today

                    override fun isSelectableYear(year: Int) = year <= today.year
                },
        )
    DatePickerDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(
                onClick = {
                    state.selectedDateMillis?.let { onChosen(it.toUtcDay()) }
                    onDismiss()
                },
            ) {
                Text("Set date")
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    ) {
        DatePicker(state = state)
    }
}

private fun Long.toUtcDay(): LocalDate = Instant.ofEpochMilli(this).atZone(ZoneOffset.UTC).toLocalDate()

/** Finishing never silently records or drops a row: this names what is left out and asks. */
@Composable
private fun FinishReview(workout: ActiveWorkout, onReturn: () -> Unit, onOmitAndFinish: () -> Unit) {
    val confirmed = workout.entries.sumOf { entry -> entry.sets.count { it.completed } }
    val unchecked =
        workout.entries.mapNotNull { entry ->
            entry.sets.filterNot { it.completed }.takeIf { it.isNotEmpty() }?.let { sets ->
                "${entry.exerciseName}: ${pluralize(sets.size, "unchecked Set")}"
            }
        }
    AlertDialog(
        onDismissRequest = onReturn,
        title = { Text(if (confirmed == 0) "No confirmed Sets" else "Finish with unchecked Sets?") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    if (confirmed == 0) {
                        "Check at least one performed Set to finish this Workout."
                    } else {
                        "${pluralize(confirmed, "confirmed Set")} will be recorded. These will be left out:"
                    },
                )
                if (confirmed > 0) unchecked.forEach { Text(it) }
            }
        },
        confirmButton = {
            if (confirmed > 0) TextButton(onClick = onOmitAndFinish) { Text("Omit and finish") }
        },
        dismissButton = { TextButton(onClick = onReturn) { Text("Return to logging") } },
    )
}
