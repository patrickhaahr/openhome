package com.patrickhaahr.openhome.ui.workout

import android.database.SQLException
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.withResumed
import com.patrickhaahr.openhome.R
import com.patrickhaahr.openhome.fitness.CorrectionDraft
import com.patrickhaahr.openhome.fitness.CorrectionEntry
import com.patrickhaahr.openhome.fitness.CorrectionSave
import com.patrickhaahr.openhome.fitness.CorrectionSet
import com.patrickhaahr.openhome.fitness.PickedExercise
import com.patrickhaahr.openhome.fitness.SetCheck
import com.patrickhaahr.openhome.fitness.SetDraft
import com.patrickhaahr.openhome.fitness.WorkoutCorrections
import com.patrickhaahr.openhome.storage.SetMeasure
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.formatDay
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

private val SET_NUMBER_WIDTH = 28.dp
private val REMOVE_WIDTH = 48.dp

/** The form as the user left it; [showMissing] once saving has found rows still to fill in. */
@Serializable
private data class CorrectionForm(
    val original: CorrectionDraft,
    val draft: CorrectionDraft,
    val showMissing: Boolean = false,
) {
    val isChanged: Boolean
        get() = draft != original
}

/** Whole in saved state, so Android reclaiming the process does not lose what has been typed. */
private val FORM_SAVER =
    Saver<CorrectionForm?, String>(
        save = { form -> form?.let { Json.encodeToString(it) } },
        restore = { Json.decodeFromString(it) },
    )

/** Saves in the app's scope: once the user has pressed Save, leaving the screen cannot lose it. */
private class WorkoutCorrectionViewModel(
    private val corrections: WorkoutCorrections,
    private val saveScope: CoroutineScope,
    private val workoutId: Long,
) : ViewModel() {
    private val savedNow = MutableStateFlow(false)
    private val problemNow = MutableStateFlow<String?>(null)
    private var saving = false

    val saved: StateFlow<Boolean> = savedNow
    val problem: StateFlow<String?> = problemNow

    suspend fun load(): CorrectionDraft? = corrections.load(workoutId)

    suspend fun pick(exerciseId: Long): PickedExercise? = corrections.pick(exerciseId)

    fun save(draft: CorrectionDraft, onInvalidSets: () -> Unit) {
        if (saving) return
        saving = true
        problemNow.value = null
        saveScope.launch {
            try {
                when (corrections.save(workoutId, draft)) {
                    CorrectionSave.Saved -> savedNow.value = true
                    CorrectionSave.InvalidSets -> {
                        problemNow.value = "Fix the marked Sets to save."
                        onInvalidSets()
                    }
                    CorrectionSave.NoSetsLeft -> problemNow.value = "A Workout needs at least one Set."
                    CorrectionSave.WorkoutGone -> problemNow.value = "This Workout isn't saved on this device."
                }
            } catch (_: SQLException) {
                problemNow.value = "Couldn't save on this device. Please try again."
            } finally {
                saving = false
            }
        }
    }
}

/**
 * Editing a finished Workout. Nothing changes until Save, which needs no connection; back with unsaved
 * changes asks before dropping them. [pickedExercise] is what the picker chose, to be added here.
 */
@Composable
fun WorkoutCorrectionRoute(
    workoutId: Long,
    pickedExercise: Long?,
    onPickedExerciseUsed: () -> Unit,
    onAddExercise: () -> Unit,
    onDone: () -> Unit,
) {
    val viewModel = appViewModel { WorkoutCorrectionViewModel(it.corrections, it.saveScope, workoutId) }
    val saved by viewModel.saved.collectAsStateWithLifecycle()
    val problem by viewModel.problem.collectAsStateWithLifecycle()
    var form by rememberSaveable(stateSaver = FORM_SAVER) { mutableStateOf<CorrectionForm?>(null) }
    var workoutGone by rememberSaveable { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        if (form == null) {
            form = viewModel.load()?.let { CorrectionForm(original = it, draft = it) }
            workoutGone = form == null
        }
    }
    LaunchedEffect(pickedExercise, form == null) {
        val current = form
        if (pickedExercise != null && current != null) {
            viewModel.pick(pickedExercise)?.let { picked ->
                form = form?.let { it.copy(draft = it.draft.addingEntry(picked)) }
            }
            onPickedExerciseUsed()
        }
    }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(saved) {
        if (saved) lifecycle.withResumed { onDone() }
    }

    if (workoutGone) {
        EmptyState(title = "Workout not found", message = "This Workout isn't saved on this device.")
    }
    form?.let { current ->
        CorrectionContent(
            form = current,
            problem = problem,
            onChange = { change -> form = form?.let { it.copy(draft = change(it.draft)) } },
            onAddExercise = onAddExercise,
            onSave = {
                if (current.isChanged) {
                    viewModel.save(current.draft, onInvalidSets = { form = form?.copy(showMissing = true) })
                } else {
                    onDone()
                }
            },
            onDiscard = onDone,
        )
    }
}

@Composable
private fun CorrectionContent(
    form: CorrectionForm,
    problem: String?,
    onChange: ((CorrectionDraft) -> CorrectionDraft) -> Unit,
    onAddExercise: () -> Unit,
    onSave: () -> Unit,
    onDiscard: () -> Unit,
) {
    val draft = form.draft
    var choosingDate by rememberSaveable { mutableStateOf(false) }
    var confirmingDiscard by rememberSaveable { mutableStateOf(false) }
    val focusManager = LocalFocusManager.current

    BackHandler(enabled = form.isChanged) { confirmingDiscard = true }

    Column(Modifier.fillMaxSize().imePadding()) {
        Column(
            modifier = Modifier.weight(1f).verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            ScreenHeader(title = "Edit Workout", subtitle = "Saves on this device first, then syncs")
            Column(
                modifier = Modifier.padding(horizontal = 16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(onClick = { choosingDate = true }, modifier = Modifier.heightIn(min = 48.dp)) {
                    Text(formatDay(draft.date))
                }
                OutlinedTextField(
                    value = draft.name,
                    onValueChange = { text -> onChange { it.copy(name = text) } },
                    label = { Text("Name (optional)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = draft.notes,
                    onValueChange = { text -> onChange { it.copy(notes = text) } },
                    label = { Text("Notes (optional)") },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            draft.entries.forEachIndexed { index, entry ->
                key(entry.key) {
                    EntryCard(
                        entry = entry,
                        isFirst = index == 0,
                        isLast = index == draft.entries.lastIndex,
                        showMissing = form.showMissing,
                        onChange = onChange,
                    )
                }
            }
            OutlinedButton(
                onClick = onAddExercise,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp).heightIn(min = 48.dp),
            ) {
                Text("Add Exercise")
            }
        }
        problem?.let {
            Text(
                text = it,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 8.dp),
            )
        }
        Button(
            onClick = {
                focusManager.clearFocus()
                onSave()
            },
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp).heightIn(min = 56.dp),
        ) {
            Text(text = "Save Changes", style = MaterialTheme.typography.titleMedium)
        }
    }

    if (choosingDate) {
        WorkoutDatePicker(
            date = draft.date,
            onChosen = { date -> onChange { it.copy(date = date.toString()) } },
            onDismiss = { choosingDate = false },
        )
    }
    if (confirmingDiscard) {
        AlertDialog(
            onDismissRequest = { confirmingDiscard = false },
            title = { Text("Discard your changes?") },
            text = { Text("This Workout stays as it was saved.") },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmingDiscard = false
                        onDiscard()
                    },
                ) {
                    Text("Discard changes")
                }
            },
            dismissButton = { TextButton(onClick = { confirmingDiscard = false }) { Text("Keep editing") } },
        )
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun EntryCard(
    entry: CorrectionEntry,
    isFirst: Boolean,
    isLast: Boolean,
    showMissing: Boolean,
    onChange: ((CorrectionDraft) -> CorrectionDraft) -> Unit,
) {
    val name = entry.exerciseName
    val selectedMeasure = entry.sets.map { it.values.measure }.distinct().singleOrNull()
    val showsReps = entry.sets.any { it.values.measure.usesReps }
    val showsSeconds = entry.sets.any { it.values.measure.usesSeconds }
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainer,
        shape = MaterialTheme.shapes.large,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
    ) {
        Column(Modifier.padding(start = 16.dp, end = 8.dp, top = 12.dp, bottom = 12.dp)) {
            Text(text = name, style = MaterialTheme.typography.titleMedium)
            entry.notes?.let {
                Text(
                    text = it,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            FlowRow {
                if (!isFirst) {
                    TextAction("Move up", "Move $name up") { onChange { it.movingEntry(entry.key, -1) } }
                }
                if (!isLast) {
                    TextAction("Move down", "Move $name down") { onChange { it.movingEntry(entry.key, 1) } }
                }
                TextAction("Remove", "Remove $name") { onChange { it.removingEntry(entry.key) } }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                MeasureChip(name, SetMeasure.REPS, "Reps", selectedMeasure) { measure -> onChange { it.settingMeasure(entry.key, measure) } }
                MeasureChip(name, SetMeasure.HOLD, "Hold", selectedMeasure) { measure -> onChange { it.settingMeasure(entry.key, measure) } }
            }
            Row(
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                ColumnLabel("Set", Modifier.width(SET_NUMBER_WIDTH))
                if (showsReps) ColumnLabel("Reps", Modifier.weight(1f))
                if (showsSeconds) ColumnLabel("Seconds", Modifier.weight(1f))
                ColumnLabel("Added kg", Modifier.weight(1f))
                ColumnLabel("RPE", Modifier.weight(1f))
                Box(Modifier.width(REMOVE_WIDTH))
            }
            entry.sets.forEachIndexed { index, set ->
                key(set.key) {
                    SetRow(
                        label = "$name set ${index + 1}",
                        number = index + 1,
                        set = set,
                        showsReps = showsReps,
                        showsSeconds = showsSeconds,
                        showMissing = showMissing,
                        onValues = { change -> onChange { it.updatingSet(entry.key, set.key, change) } },
                        onRemove =
                            if (entry.sets.size > 1) {
                                { onChange { it.removingSet(entry.key, set.key) } }
                            } else {
                                null
                            },
                    )
                }
            }
            TextAction("Add set", "Add set to $name") { onChange { it.addingSet(entry.key) } }
        }
    }
}

@Composable
private fun TextAction(text: String, description: String, onClick: () -> Unit) {
    TextButton(onClick = onClick, modifier = Modifier.semantics { contentDescription = description }) {
        Text(text)
    }
}

@Composable
private fun MeasureChip(
    exerciseName: String,
    measure: SetMeasure,
    label: String,
    selected: SetMeasure?,
    onSelect: (SetMeasure) -> Unit,
) {
    FilterChip(
        selected = selected == measure,
        onClick = { onSelect(measure) },
        label = { Text(label) },
        modifier = Modifier.semantics { contentDescription = "Log $exerciseName as $label" },
    )
}

@Composable
private fun SetRow(
    label: String,
    number: Int,
    set: CorrectionSet,
    showsReps: Boolean,
    showsSeconds: Boolean,
    showMissing: Boolean,
    onValues: ((SetDraft) -> SetDraft) -> Unit,
    onRemove: (() -> Unit)?,
) {
    val values = set.values
    val problem = values.check() as? SetCheck.Problem
    Column {
        Row(
            modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = number.toString(),
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.width(SET_NUMBER_WIDTH),
            )
            if (values.measure.usesReps) {
                ValueField(values.reps, "$label reps", KeyboardType.Number) { text -> onValues { it.copy(reps = text) } }
            } else if (showsReps) {
                Spacer(Modifier.weight(1f))
            }
            if (values.measure.usesSeconds) {
                ValueField(values.durationSeconds, "$label seconds", KeyboardType.Number) { text ->
                    onValues { it.copy(durationSeconds = text) }
                }
            } else if (showsSeconds) {
                Spacer(Modifier.weight(1f))
            }
            ValueField(values.addedWeightKg, "$label added kg", KeyboardType.Decimal) { text ->
                onValues { it.copy(addedWeightKg = text) }
            }
            ValueField(values.rpe, "$label RPE", KeyboardType.Number) { text -> onValues { it.copy(rpe = text) } }
            Box(Modifier.width(REMOVE_WIDTH), contentAlignment = Alignment.Center) {
                if (onRemove != null) {
                    IconButton(onClick = onRemove) {
                        Icon(painterResource(R.drawable.ic_close), contentDescription = "Remove $label")
                    }
                }
            }
        }
        if (problem != null && (!problem.isMissingValue || showMissing)) {
            Text(
                text = problem.message,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(start = SET_NUMBER_WIDTH + 8.dp, bottom = 4.dp),
            )
        }
        set.notes?.let {
            Text(
                text = it,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = SET_NUMBER_WIDTH + 8.dp, bottom = 4.dp),
            )
        }
    }
}

@Composable
private fun RowScope.ValueField(
    value: String,
    description: String,
    keyboard: KeyboardType,
    onValueChange: (String) -> Unit,
) {
    CompactValueField(
        value = value,
        onValueChange = onValueChange,
        description = description,
        keyboard = keyboard,
        highlighted = false,
        modifier = Modifier.weight(1f),
    )
}
