package com.patrickhaahr.openhome.ui.workout

import android.database.SQLException
import android.os.SystemClock
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.ActiveEntry
import com.patrickhaahr.openhome.fitness.ActiveSet
import com.patrickhaahr.openhome.fitness.ActiveWorkout
import com.patrickhaahr.openhome.fitness.ActiveWorkoutRepository
import com.patrickhaahr.openhome.fitness.FinishOutcome
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.fitness.PreviousPerformance
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
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

private val SET_NUMBER_WIDTH = 28.dp
private val CONFIRM_WIDTH = 48.dp
private const val COMPLETE_GUARD_MILLIS = 400L

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
    fitness: FitnessRepository,
    private val saveScope: CoroutineScope,
) : ViewModel() {
    private val writesInProgress = MutableStateFlow(0)
    private val lastWriteFailed = MutableStateFlow(false)
    private val refusedConfirmations = MutableStateFlow(emptySet<Long>())

    val workout: StateFlow<ActiveLookup> =
        repository.observe()
            .map<ActiveWorkout?, ActiveLookup> { ActiveLookup.Loaded(it) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ActiveLookup.Loading)

    /** May briefly be the previous Exercise's after moving on, so check whose it is before showing it. */
    @OptIn(ExperimentalCoroutinesApi::class)
    val previousPerformance: StateFlow<PreviousPerformance?> =
        workout
            .map { (it as? ActiveLookup.Loaded)?.workout?.currentEntry?.exerciseId }
            .distinctUntilChanged()
            .flatMapLatest { exerciseId ->
                if (exerciseId == null) flowOf(null) else fitness.observePreviousPerformance(exerciseId)
            }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

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

    fun goToEntry(entryId: Long) = save { repository.goToEntry(entryId) }

    fun addSet(entryId: Long) = save { repository.addSet(entryId) }

    fun repeatLastSet(entryId: Long) = save { repository.repeatLastSet(entryId) }

    fun removeLastSet(entryId: Long) = save { repository.removeLastSet(entryId) }

    fun completeExercise(entryId: Long) = save {
        val completion = repository.completeExercise(entryId)
        refusedConfirmations.update { it + completion.refused }
    }

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
    val viewModel = appViewModel { ActiveWorkoutViewModel(it.activeWorkout, it.fitness, it.saveScope) }
    val lookup by viewModel.workout.collectAsStateWithLifecycle()
    val saveState by viewModel.saveState.collectAsStateWithLifecycle()
    val refusedSets by viewModel.refusedSets.collectAsStateWithLifecycle()
    val previousPerformance by viewModel.previousPerformance.collectAsStateWithLifecycle()

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
                        previousPerformance = previousPerformance,
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
    previousPerformance: PreviousPerformance?,
    viewModel: ActiveWorkoutViewModel,
    onAddExercise: () -> Unit,
    onFinished: () -> Unit,
    onDiscarded: () -> Unit,
) {
    var choosingDate by rememberSaveable { mutableStateOf(false) }
    var reviewingFinish by rememberSaveable { mutableStateOf(false) }
    var confirmingDiscard by rememberSaveable { mutableStateOf(false) }
    val current = workout.currentEntry
    val listState = rememberLazyListState()

    // A new Exercise starts from its top, wherever the last one had been scrolled to.
    var shownEntryId by rememberSaveable { mutableStateOf(current?.id) }
    LaunchedEffect(current?.id) {
        if (current?.id != shownEntryId) {
            shownEntryId = current?.id
            listState.scrollToItem(0)
        }
    }

    Column(Modifier.fillMaxSize().imePadding()) {
        LazyColumn(
            state = listState,
            modifier = Modifier.weight(1f),
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
            if (current != null) {
                item {
                    ExerciseNavigator(
                        workout.entries,
                        current,
                        onGoTo = viewModel::goToEntry,
                        modifier = Modifier.padding(horizontal = 16.dp),
                    )
                }
                // Keyed, so each Exercise's rows and their typed text belong to it.
                item(key = current.id) {
                    EntryCard(current, previousPerformance, refusedSets, viewModel)
                }
            }
            item {
                Column(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    OutlinedButton(onClick = onAddExercise, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) {
                        Text("Add Exercise")
                    }
                    val finish = {
                        if (workout.hasOnlyConfirmedSets()) {
                            viewModel.finish(omitUnchecked = false, onFinished)
                        } else {
                            reviewingFinish = true
                        }
                    }
                    val finishModifier = Modifier.fillMaxWidth().heightIn(min = 56.dp)
                    if (workout.hasOnlyConfirmedSets()) {
                        Button(onClick = finish, modifier = finishModifier) {
                            Text(text = "Finish Workout", style = MaterialTheme.typography.titleMedium)
                        }
                    } else {
                        OutlinedButton(onClick = finish, modifier = finishModifier) {
                            Text(text = "Finish Workout", style = MaterialTheme.typography.titleMedium)
                        }
                    }
                    TextButton(onClick = { confirmingDiscard = true }, modifier = Modifier.fillMaxWidth()) {
                        Text(text = "Discard Workout", color = MaterialTheme.colorScheme.error)
                    }
                }
            }
        }
        // Pinned, so it stays within reach and the next Exercise cannot put another action under the finger.
        current?.let { CompleteExerciseButton(it, viewModel, Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) }
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

/** Name and notes stay out of the way while logging; both are optional. */
@Composable
private fun SessionFields(
    workout: ActiveWorkout,
    viewModel: ActiveWorkoutViewModel,
    onChooseDate: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var name by rememberSaveable { mutableStateOf(workout.name) }
    var notes by rememberSaveable { mutableStateOf(workout.notes) }
    var editing by rememberSaveable { mutableStateOf(false) }
    Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = onChooseDate, modifier = Modifier.heightIn(min = 48.dp)) {
                Text(formatDay(workout.date))
            }
            TextButton(onClick = { editing = !editing }, modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
                Text(
                    text =
                        when {
                            editing -> "Done"
                            name.isNotBlank() -> name
                            notes.isNotBlank() -> "Edit notes"
                            else -> "Add name or notes"
                        },
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (editing) {
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
}

/** Every Exercise in order and how much of it is confirmed. Going to one only moves the focus. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ExerciseNavigator(
    entries: List<ActiveEntry>,
    current: ActiveEntry,
    onGoTo: (Long) -> Unit,
    modifier: Modifier = Modifier,
) {
    FlowRow(modifier = modifier, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        entries.forEach { entry ->
            val confirmed = entry.sets.count { it.completed }
            val isCurrent = entry.id == current.id
            FilterChip(
                selected = isCurrent,
                onClick = { onGoTo(entry.id) },
                label = { Text("${entry.exerciseName} $confirmed/${entry.sets.size}") },
                modifier =
                    Modifier.semantics {
                        contentDescription = "Go to ${entry.exerciseName}"
                        val progress = "$confirmed of ${entry.sets.size} Sets confirmed"
                        stateDescription = if (isCurrent) "Current, $progress" else progress
                    },
            )
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun EntryCard(
    entry: ActiveEntry,
    previousPerformance: PreviousPerformance?,
    refusedSets: Set<Long>,
    viewModel: ActiveWorkoutViewModel,
) {
    val selectedMeasure = entry.sets.map { it.draft.measure }.distinct().singleOrNull()
    val columns = AmountColumns(entry)
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainer,
        shape = MaterialTheme.shapes.large,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
    ) {
        Column(Modifier.padding(start = 16.dp, end = 8.dp, top = 12.dp, bottom = 12.dp)) {
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
            previousPerformance?.takeIf { it.exerciseId == entry.exerciseId }?.let { PreviousPerformanceLines(it) }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                MeasureChip(entry, SetMeasure.REPS, "Reps", selectedMeasure, viewModel)
                MeasureChip(entry, SetMeasure.HOLD, "Hold", selectedMeasure, viewModel)
            }
            SetColumnLabels(columns)
            entry.sets.forEach { set ->
                SetRow(entry.exerciseName, set, columns, showMissingValue = set.id in refusedSets, viewModel)
            }
            FlowRow {
                TextButton(
                    onClick = { viewModel.repeatLastSet(entry.id) },
                    modifier = Modifier.semantics { contentDescription = "Repeat last set of ${entry.exerciseName}" },
                ) {
                    Text("Repeat last set")
                }
                TextButton(
                    onClick = { viewModel.addSet(entry.id) },
                    modifier = Modifier.semantics { contentDescription = "Add set to ${entry.exerciseName}" },
                ) {
                    Text("Add set")
                }
                if (entry.sets.size > 1) {
                    TextButton(
                        onClick = { viewModel.removeLastSet(entry.id) },
                        modifier = Modifier.semantics { contentDescription = "Remove last set of ${entry.exerciseName}" },
                    ) {
                        Text("Remove last")
                    }
                }
            }
        }
    }
}

/**
 * The explicit confirmation of everything shown. The next Exercise appears under the same button, so
 * the second half of a double tap must not confirm it unseen.
 */
@Composable
private fun CompleteExerciseButton(entry: ActiveEntry, viewModel: ActiveWorkoutViewModel, modifier: Modifier) {
    val focusManager = LocalFocusManager.current
    val shownAt = remember(entry.id) { SystemClock.uptimeMillis() }
    val onClick = {
        if (SystemClock.uptimeMillis() - shownAt >= COMPLETE_GUARD_MILLIS) {
            focusManager.clearFocus()
            viewModel.completeExercise(entry.id)
        }
    }
    val fullWidth =
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .semantics { contentDescription = "Complete ${entry.exerciseName}" }
    if (entry.sets.all { it.completed }) {
        OutlinedButton(onClick = onClick, modifier = fullWidth) {
            Text(text = "Complete Exercise", style = MaterialTheme.typography.titleMedium)
        }
    } else {
        Button(onClick = onClick, modifier = fullWidth) {
            Text(text = "Complete Exercise", style = MaterialTheme.typography.titleMedium)
        }
    }
}

@Composable
private fun PreviousPerformanceLines(previous: PreviousPerformance) {
    Column(Modifier.padding(top = 8.dp, bottom = 8.dp, end = 8.dp)) {
        Text(
            text = "Last time · ${formatDay(previous.date)}",
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        previous.sets.forEachIndexed { index, set ->
            Text(
                text = listOfNotNull(
                    "Set ${index + 1}: ${describePerformance(set)}",
                    set.rpe?.let { "RPE $it" },
                ).joinToString(separator = " · "),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun MeasureChip(
    entry: ActiveEntry,
    measure: SetMeasure,
    label: String,
    selected: SetMeasure?,
    viewModel: ActiveWorkoutViewModel,
) {
    FilterChip(
        selected = selected == measure,
        onClick = { viewModel.setMeasure(entry.id, measure) },
        label = { Text(label) },
        modifier = Modifier.semantics { contentDescription = "Log ${entry.exerciseName} as $label" },
    )
}

/** Almost always one amount column; an entry mixing reps and holds keeps its columns aligned. */
private class AmountColumns(entry: ActiveEntry) {
    val reps = entry.sets.any { it.draft.measure.usesReps }
    val seconds = entry.sets.any { it.draft.measure.usesSeconds }
}

@Composable
private fun SetColumnLabels(columns: AmountColumns) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        ColumnLabel("Set", Modifier.width(SET_NUMBER_WIDTH))
        if (columns.reps) ColumnLabel("Reps", Modifier.weight(1f))
        if (columns.seconds) ColumnLabel("Seconds", Modifier.weight(1f))
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
private fun SetRow(
    exerciseName: String,
    set: ActiveSet,
    columns: AmountColumns,
    showMissingValue: Boolean,
    viewModel: ActiveWorkoutViewModel,
) {
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
            if (set.draft.measure.usesReps) {
                SetValueField(set, SetField.REPS, set.draft.reps, "$label reps", KeyboardType.Number, viewModel)
            } else if (columns.reps) {
                Spacer(Modifier.weight(1f))
            }
            if (set.draft.measure.usesSeconds) {
                SetValueField(
                    set,
                    SetField.DURATION,
                    set.draft.durationSeconds,
                    "$label seconds",
                    KeyboardType.Number,
                    viewModel,
                )
            } else if (columns.seconds) {
                Spacer(Modifier.weight(1f))
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
