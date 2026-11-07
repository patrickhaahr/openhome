package com.patrickhaahr.openhome.ui.workout

import android.database.SQLException
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.withResumed
import com.patrickhaahr.openhome.fitness.CorrectionDraft
import com.patrickhaahr.openhome.fitness.CorrectionEntry
import com.patrickhaahr.openhome.fitness.CorrectionSave
import com.patrickhaahr.openhome.fitness.CorrectionSet
import com.patrickhaahr.openhome.fitness.PickedExercise
import com.patrickhaahr.openhome.fitness.SetCheck
import com.patrickhaahr.openhome.fitness.SetDraft
import com.patrickhaahr.openhome.fitness.WorkoutCorrections
import com.patrickhaahr.openhome.ui.DayPicker
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.FormField
import com.patrickhaahr.openhome.ui.ProblemText
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.SlabKey
import com.patrickhaahr.openhome.ui.design.key as tappable
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

private val SET_NUMBER_WIDTH = 24.dp

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
        EmptyState(title = "Workout not found", message = "This Workout isn't saved on this device.", glyph = Glyph.Rewind)
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
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            ScreenHeader(title = "Edit Workout", glyph = Glyph.Pencil, subtitle = "Saves on this device first, then syncs")
            Column(
                modifier = Modifier.padding(horizontal = ScreenMargin),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                DateKey(draft.date) { choosingDate = true }
                FormField(
                    value = draft.name,
                    onValueChange = { text -> onChange { it.copy(name = text) } },
                    label = "Name (optional)",
                    glyph = Glyph.Pencil,
                )
                FormField(
                    value = draft.notes,
                    onValueChange = { text -> onChange { it.copy(notes = text) } },
                    label = "Notes (optional)",
                    glyph = Glyph.Note,
                    singleLine = false,
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
            Row(Modifier.fillMaxWidth().padding(horizontal = ScreenMargin), horizontalArrangement = Arrangement.End) {
                GlyphKey(Glyph.Plus, "Add Exercise", onAddExercise, tone = KeyTone.Slab, size = 64.dp)
            }
            Spacer(Modifier.height(4.dp))
        }
        problem?.let { ProblemText(it, Modifier.padding(start = 20.dp, end = 20.dp, top = 8.dp)) }
        SlabKey(
            Glyph.Check,
            "Save Changes",
            {
                focusManager.clearFocus()
                onSave()
            },
            modifier = Modifier.fillMaxWidth().padding(horizontal = ScreenMargin, vertical = 8.dp),
        )
    }

    if (choosingDate) {
        DayPicker(
            date = draft.date,
            onChosen = { date -> onChange { it.copy(date = date.toString()) } },
            onDismiss = { choosingDate = false },
        )
    }
    if (confirmingDiscard) {
        KeyDialog(
            onDismiss = { confirmingDiscard = false },
            title = "Discard your changes?",
            message = "This Workout stays as it was saved.",
            glyph = Glyph.Pencil,
            confirm = {
                GlyphKey(
                    Glyph.Trash,
                    "Discard changes",
                    {
                        confirmingDiscard = false
                        onDiscard()
                    },
                    tone = KeyTone.Alarm,
                )
            },
            dismiss = { GlyphKey(Glyph.Back, "Keep editing", { confirmingDiscard = false }) },
        )
    }
}

@Composable
private fun DateKey(date: String, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .height(64.dp)
            .tappable(formatDay(date), onClick, shape = com.patrickhaahr.openhome.ui.FieldShape, depth = 0.98f)
            .background(Lit.Slab, com.patrickhaahr.openhome.ui.FieldShape)
            .padding(horizontal = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        GlyphIcon(Glyph.Calendar, tint = Lit.Lamp, size = 26.dp)
        Text(formatDay(date), style = Type.Name, color = Lit.Ink)
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
    Slab(Modifier.fillMaxWidth().padding(horizontal = ScreenMargin), padding = 12.dp, arrangement = Arrangement.spacedBy(8.dp)) {
        Text(text = name, style = Type.Figure, color = Lit.Ink, modifier = Modifier.padding(start = 4.dp, top = 2.dp))
        entry.notes?.let { Text(text = it, style = Type.Body, color = Lit.Muted, modifier = Modifier.padding(start = 4.dp)) }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            MeasureSwitch(selectedMeasure, name) { measure -> onChange { it.settingMeasure(entry.key, measure) } }
            Spacer(Modifier.weight(1f))
            if (!isFirst) GlyphKey(Glyph.ArrowUp, "Move $name up", { onChange { it.movingEntry(entry.key, -1) } }, tone = KeyTone.Quiet, size = 48.dp)
            if (!isLast) GlyphKey(Glyph.ArrowDown, "Move $name down", { onChange { it.movingEntry(entry.key, 1) } }, tone = KeyTone.Quiet, size = 48.dp)
            GlyphKey(Glyph.Trash, "Remove $name", { onChange { it.removingEntry(entry.key) } }, tone = KeyTone.Quiet, size = 48.dp)
        }
        entry.sets.forEachIndexed { index, set ->
            key(set.key) {
                SetRow(
                    label = "$name set ${index + 1}",
                    number = index + 1,
                    set = set,
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
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            GlyphKey(Glyph.Plus, "Add set to $name", { onChange { it.addingSet(entry.key) } }, tone = KeyTone.Raised, size = 52.dp)
        }
    }
}

@Composable
private fun SetRow(
    label: String,
    number: Int,
    set: CorrectionSet,
    showMissing: Boolean,
    onValues: ((SetDraft) -> SetDraft) -> Unit,
    onRemove: (() -> Unit)?,
) {
    val values = set.values
    val problem = values.check() as? SetCheck.Problem
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                text = number.toString(),
                style = Type.Name,
                color = Lit.Muted,
                modifier = Modifier.width(SET_NUMBER_WIDTH),
            )
            if (values.measure.usesReps) {
                ValueField(values.reps, "$label reps", "×", KeyboardType.Number) { text -> onValues { it.copy(reps = text) } }
            }
            if (values.measure.usesSeconds) {
                ValueField(values.durationSeconds, "$label seconds", "s", KeyboardType.Number) { text ->
                    onValues { it.copy(durationSeconds = text) }
                }
            }
            ValueField(values.addedWeightKg, "$label added kg", "kg", KeyboardType.Decimal) { text ->
                onValues { it.copy(addedWeightKg = text) }
            }
            ValueField(values.rpe, "$label RPE", "rpe", KeyboardType.Number) { text -> onValues { it.copy(rpe = text) } }
            if (onRemove != null) {
                GlyphKey(Glyph.Close, "Remove $label", onRemove, tone = KeyTone.Quiet, size = 44.dp)
            } else {
                Spacer(Modifier.width(44.dp))
            }
        }
        if (problem != null && (!problem.isMissingValue || showMissing)) {
            ProblemText(problem.message, Modifier.padding(start = SET_NUMBER_WIDTH, bottom = 4.dp))
        }
        set.notes?.let {
            Text(text = it, style = Type.Body, color = Lit.Muted, modifier = Modifier.padding(start = SET_NUMBER_WIDTH + 6.dp, bottom = 4.dp))
        }
    }
}

@Composable
private fun RowScope.ValueField(
    value: String,
    description: String,
    unit: String,
    keyboard: KeyboardType,
    onValueChange: (String) -> Unit,
) {
    CompactValueField(
        value = value,
        onValueChange = onValueChange,
        description = description,
        unit = unit,
        keyboard = keyboard,
        highlighted = false,
        modifier = Modifier.weight(1f),
    )
}
