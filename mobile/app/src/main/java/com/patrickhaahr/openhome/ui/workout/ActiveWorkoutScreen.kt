package com.patrickhaahr.openhome.ui.workout

import android.database.SQLException
import android.os.SystemClock
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.interaction.MutableInteractionSource
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
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
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
import com.patrickhaahr.openhome.fitness.REST_STEP_SECONDS
import com.patrickhaahr.openhome.fitness.RestTimers
import com.patrickhaahr.openhome.fitness.SetCheck
import com.patrickhaahr.openhome.fitness.SetField
import com.patrickhaahr.openhome.storage.SetMeasure
import com.patrickhaahr.openhome.ui.DayPicker
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.FormField
import com.patrickhaahr.openhome.ui.ProblemText
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.design.FigureKey
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.MarkDot
import com.patrickhaahr.openhome.ui.design.Motion
import com.patrickhaahr.openhome.ui.design.SegmentRing
import com.patrickhaahr.openhome.ui.design.SlabKey
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.design.monogramOf
import com.patrickhaahr.openhome.ui.design.pressable
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.formatShortDay
import com.patrickhaahr.openhome.ui.pluralize
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import java.time.LocalDate
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

private val RAIL_RING = 52.dp
private val CONFIRM = 52.dp
private val CELL_HEIGHT = 68.dp
private val SET_SHAPE = RoundedCornerShape(24.dp)
private val CELL_SHAPE = RoundedCornerShape(16.dp)
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
    private val restTimers: RestTimers,
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

    fun adjustRest(exerciseId: Long, deltaSeconds: Long) = save { restTimers.adjust(exerciseId, deltaSeconds) }

    fun setRestEnabled(exerciseId: Long, enabled: Boolean) = save { restTimers.setEnabled(exerciseId, enabled) }

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
    val viewModel = appViewModel { ActiveWorkoutViewModel(it.activeWorkout, it.fitness, it.restTimers, it.saveScope) }
    val lookup by viewModel.workout.collectAsStateWithLifecycle()
    val saveState by viewModel.saveState.collectAsStateWithLifecycle()
    val refusedSets by viewModel.refusedSets.collectAsStateWithLifecycle()
    val previousPerformance by viewModel.previousPerformance.collectAsStateWithLifecycle()

    when (val current = lookup) {
        ActiveLookup.Loading -> Unit
        is ActiveLookup.Loaded ->
            when (val workout = current.workout) {
                null -> EmptyState(title = "No Active Workout", message = "Start one from the Workout tab.", glyph = Glyph.Barbell)
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
        // Pinned: however far logging has scrolled, the way out of the session stays in sight.
        SessionBar(
            workout = workout,
            saveState = saveState,
            viewModel = viewModel,
            onChooseDate = { choosingDate = true },
            onDiscard = { confirmingDiscard = true },
            onFinish = {
                if (workout.hasOnlyConfirmedSets()) {
                    viewModel.finish(omitUnchecked = false, onFinished)
                } else {
                    reviewingFinish = true
                }
            },
        )
        LazyColumn(
            state = listState,
            modifier = Modifier.weight(1f),
            contentPadding = PaddingValues(top = 16.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            if (current == null) {
                // Nothing to log yet, so adding the first Exercise is all this screen is for.
                item {
                    Box(Modifier.fillParentMaxHeight(0.8f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                        GlyphKey(Glyph.Plus, "Add Exercise", onAddExercise, tone = KeyTone.Lamp, size = 220.dp, glyphSize = 72.dp)
                    }
                }
            } else {
                item {
                    ExerciseRail(
                        workout.entries,
                        current,
                        onGoTo = viewModel::goToEntry,
                        onAddExercise = onAddExercise,
                        modifier = Modifier.padding(horizontal = ScreenMargin),
                    )
                }
            }
            if (current != null) {
                // Keyed, so each Exercise's rows and their typed text belong to it.
                item(key = current.id) {
                    ExerciseFocus(current, previousPerformance, refusedSets, viewModel)
                }
            }
        }
        // Pinned, so it stays within reach and the next Exercise cannot put another action under the finger.
        current?.let {
            CompleteExerciseButton(it, viewModel, Modifier.padding(horizontal = ScreenMargin, vertical = 8.dp))
        }
    }

    if (choosingDate) {
        DayPicker(
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
        KeyDialog(
            onDismiss = { confirmingDiscard = false },
            title = "Discard this Workout?",
            message = "Its Exercises and Sets will be deleted from this device.",
            glyph = Glyph.Trash,
            confirm = {
                GlyphKey(
                    Glyph.Trash,
                    "Discard",
                    {
                        confirmingDiscard = false
                        viewModel.discard(onDiscarded)
                    },
                    tone = KeyTone.Alarm,
                )
            },
            dismiss = { GlyphKey(Glyph.Back, "Keep logging", { confirmingDiscard = false }) },
        )
    }
}

private fun ActiveWorkout.hasOnlyConfirmedSets(): Boolean =
    entries.isNotEmpty() && entries.all { entry -> entry.sets.all { it.completed } }

/** A dialog that shows what is at stake and offers two symbols; the words stay with assistive technology. */
@Composable
internal fun KeyDialog(
    onDismiss: () -> Unit,
    title: String,
    message: String,
    glyph: Glyph,
    confirm: @Composable () -> Unit,
    dismiss: @Composable () -> Unit,
    content: @Composable () -> Unit = {},
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = Lit.Slab,
        icon = { GlyphIcon(glyph, tint = Lit.Lamp, size = 48.dp) },
        title = { Spoken(title) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Spoken(message)
                content()
            }
        },
        confirmButton = { confirm() },
        dismissButton = { dismiss() },
    )
}

/** The session's head: its day, its name and notes, and the two ways out. Saving is only ever a mark. */
@Composable
private fun SessionBar(
    workout: ActiveWorkout,
    saveState: SaveState,
    viewModel: ActiveWorkoutViewModel,
    onChooseDate: () -> Unit,
    onDiscard: () -> Unit,
    onFinish: () -> Unit,
) {
    var name by rememberSaveable { mutableStateOf(workout.name) }
    var notes by rememberSaveable { mutableStateOf(workout.notes) }
    var editing by rememberSaveable { mutableStateOf(false) }
    Column(
        Modifier.fillMaxWidth().padding(start = ScreenMargin, end = ScreenMargin, top = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Spoken("Active Workout", isHeading = true)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(
                Modifier
                    .height(48.dp)
                    .key(formatDay(workout.date), onChooseDate, shape = CircleShape)
                    .background(Lit.Slab, CircleShape)
                    .padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                GlyphIcon(Glyph.Calendar, tint = Lit.Muted, size = 18.dp)
                Text(formatShortDay(workout.date), style = Type.Name, color = Lit.Ink)
            }
            GlyphKey(
                if (editing) Glyph.Check else if (notes.isNotBlank() && name.isBlank()) Glyph.Note else Glyph.Pencil,
                when {
                    editing -> "Done"
                    name.isNotBlank() -> name
                    notes.isNotBlank() -> "Edit notes"
                    else -> "Add name or notes"
                },
                { editing = !editing },
                tone = if (editing) KeyTone.Lamp else KeyTone.Slab,
                size = 48.dp,
            )
            SaveMark(saveState)
            Spacer(Modifier.weight(1f))
            GlyphKey(Glyph.Trash, "Discard Workout", onDiscard, tone = KeyTone.Quiet, size = 48.dp)
            GlyphKey(
                Glyph.Flag,
                "Finish Workout",
                onFinish,
                tone = if (workout.hasOnlyConfirmedSets()) KeyTone.Lamp else KeyTone.Raised,
                size = 56.dp,
            )
        }
        if (saveState == SaveState.FAILED) ProblemText("Couldn't save on device. Your last change may be lost.")
        if (editing) {
            FormField(
                value = name,
                onValueChange = {
                    name = it
                    viewModel.setName(it)
                },
                label = "Name (optional)",
                glyph = Glyph.Pencil,
            )
            FormField(
                value = notes,
                onValueChange = {
                    notes = it
                    viewModel.setNotes(it)
                },
                label = "Notes (optional)",
                glyph = Glyph.Note,
                singleLine = false,
            )
        }
    }
}

@Composable
private fun SaveMark(state: SaveState) {
    val label =
        when (state) {
            SaveState.SAVING -> "Saving on device"
            SaveState.SAVED -> "Saved on device"
            SaveState.FAILED -> "Couldn't save on device. Your last change may be lost."
        }
    val color by animateColorAsState(
        when (state) {
            SaveState.SAVING -> Lit.Lamp
            SaveState.SAVED -> Lit.Lifted
            SaveState.FAILED -> Lit.Alarm
        },
        tween(Motion.QUICK),
        label = "save mark",
    )
    Box(Modifier.size(20.dp).semantics { contentDescription = label }, contentAlignment = Alignment.Center) {
        MarkDot(color)
    }
}

/** Every Exercise of the session as a little plate that lights as it fills. Going to one only moves the focus. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ExerciseRail(
    entries: List<ActiveEntry>,
    current: ActiveEntry?,
    onGoTo: (Long) -> Unit,
    onAddExercise: () -> Unit,
    modifier: Modifier = Modifier,
) {
    FlowRow(
        modifier = modifier,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        entries.forEach { entry ->
            val confirmed = entry.sets.count { it.completed }
            val isCurrent = entry.id == current?.id
            val complete = entry.sets.isNotEmpty() && confirmed == entry.sets.size
            Box(
                Modifier
                    .size(RAIL_RING)
                    .key("Go to ${entry.exerciseName}", { onGoTo(entry.id) })
                    .background(if (isCurrent) Lit.Lamp else Color.Transparent, CircleShape)
                    .semantics {
                        val progress = "$confirmed of ${entry.sets.size} Sets confirmed"
                        stateDescription = if (isCurrent) "Current, $progress" else progress
                    },
                contentAlignment = Alignment.Center,
            ) {
                SegmentRing(
                    confirmed,
                    entry.sets.size,
                    Modifier.fillMaxSize().padding(if (isCurrent) 3.dp else 0.dp),
                    thickness = 4.dp,
                    lit = if (isCurrent) Lit.OnLamp else Lit.Lamp,
                    unlit = if (isCurrent) Lit.LampDeep else Lit.Lifted,
                )
                if (complete && !isCurrent) {
                    GlyphIcon(Glyph.Check, tint = Lit.Lamp, size = 22.dp)
                } else {
                    Text(
                        monogramOf(entry.exerciseName),
                        style = Type.Small,
                        color = if (isCurrent) Lit.OnLamp else Lit.Muted,
                    )
                }
                Spoken("${entry.exerciseName} $confirmed/${entry.sets.size}", Modifier.align(Alignment.TopStart))
            }
        }
        GlyphKey(Glyph.Plus, "Add Exercise", onAddExercise, tone = KeyTone.Slab, size = RAIL_RING)
    }
}

/** The one Exercise being lifted, as large as the screen allows. */
@Composable
private fun ExerciseFocus(
    entry: ActiveEntry,
    previousPerformance: PreviousPerformance?,
    refusedSets: Set<Long>,
    viewModel: ActiveWorkoutViewModel,
) {
    val selectedMeasure = entry.sets.map { it.draft.measure }.distinct().singleOrNull()
    Column(Modifier.fillMaxWidth().padding(horizontal = ScreenMargin), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(verticalAlignment = Alignment.Top, modifier = Modifier.padding(start = 4.dp)) {
            Column(Modifier.weight(1f).padding(top = 2.dp)) {
                Text(entry.exerciseName, style = Type.Display, color = Lit.Ink)
                Spoken(entry.exerciseCategory)
            }
            GlyphKey(Glyph.Trash, "Remove ${entry.exerciseName}", { viewModel.removeEntry(entry.id) }, tone = KeyTone.Quiet, size = 44.dp)
        }
        previousPerformance?.takeIf { it.exerciseId == entry.exerciseId }?.let { PreviousPerformanceStrip(it) }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            entry.sets.forEach { set ->
                SetRow(entry.exerciseName, set, showMissingValue = set.id in refusedSets, viewModel)
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            MeasureSwitch(
                selected = selectedMeasure,
                exerciseName = entry.exerciseName,
                onSelect = { viewModel.setMeasure(entry.id, it) },
            )
            Spacer(Modifier.weight(1f))
            if (entry.sets.size > 1) {
                GlyphKey(Glyph.Minus, "Remove last set of ${entry.exerciseName}", { viewModel.removeLastSet(entry.id) }, tone = KeyTone.Slab)
            }
            GlyphKey(Glyph.Copy, "Repeat last set of ${entry.exerciseName}", { viewModel.repeatLastSet(entry.id) }, tone = KeyTone.Slab)
            GlyphKey(Glyph.Plus, "Add set to ${entry.exerciseName}", { viewModel.addSet(entry.id) }, tone = KeyTone.Raised)
        }
        RestPreferenceRow(entry, viewModel)
    }
}

/** Reps or a hold, by their symbols. */
@Composable
internal fun MeasureSwitch(selected: SetMeasure?, exerciseName: String, onSelect: (SetMeasure) -> Unit) {
    Row(Modifier.background(Lit.Slab, CircleShape).padding(4.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
        GlyphKey(
            Glyph.Close,
            "Log $exerciseName as Reps",
            { onSelect(SetMeasure.REPS) },
            tone = if (selected == SetMeasure.REPS) KeyTone.Raised else KeyTone.Quiet,
            size = 48.dp,
            glyphSize = 18.dp,
        )
        GlyphKey(
            Glyph.Timer,
            "Log $exerciseName as Hold",
            { onSelect(SetMeasure.HOLD) },
            tone = if (selected == SetMeasure.HOLD) KeyTone.Raised else KeyTone.Quiet,
            size = 48.dp,
            glyphSize = 22.dp,
        )
    }
}

/** What this Exercise's rests will be, remembered from Workout to Workout. */
@Composable
private fun RestPreferenceRow(entry: ActiveEntry, viewModel: ActiveWorkoutViewModel) {
    val spoken = if (entry.rest.enabled) "Rest ${formatRest(entry.rest.seconds)}" else "Rest off"
    Slab(padding = 8.dp, modifier = Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Box(
                Modifier.padding(start = 10.dp).semantics { contentDescription = spoken },
                contentAlignment = Alignment.Center,
            ) {
                GlyphIcon(Glyph.Timer, tint = if (entry.rest.enabled) Lit.Lamp else Lit.Faint, size = 24.dp)
            }
            if (entry.rest.enabled) {
                // Primes rather than a colon: the running countdown is the only m:ss on screen.
                Text(
                    "${entry.rest.seconds / 60}′${(entry.rest.seconds % 60).toString().padStart(2, '0')}",
                    style = Type.Figure,
                    color = Lit.Ink,
                    modifier = Modifier.padding(start = 4.dp),
                )
            }
            Spacer(Modifier.weight(1f))
            if (entry.rest.enabled) {
                FigureKey("−15", "Less rest for ${entry.exerciseName}", { viewModel.adjustRest(entry.exerciseId, -REST_STEP_SECONDS) }, size = 48.dp)
                FigureKey("+15", "More rest for ${entry.exerciseName}", { viewModel.adjustRest(entry.exerciseId, REST_STEP_SECONDS) }, size = 48.dp)
            }
            LampSwitch(
                checked = entry.rest.enabled,
                onCheckedChange = { viewModel.setRestEnabled(entry.exerciseId, it) },
                description = "Rest timer for ${entry.exerciseName}",
                modifier = Modifier.padding(start = 4.dp, end = 6.dp),
            )
        }
    }
}

@Composable
internal fun LampSwitch(checked: Boolean, onCheckedChange: (Boolean) -> Unit, description: String, modifier: Modifier = Modifier, enabled: Boolean = true) {
    Switch(
        checked = checked,
        onCheckedChange = onCheckedChange,
        enabled = enabled,
        colors =
            SwitchDefaults.colors(
                checkedTrackColor = Lit.Lamp,
                checkedThumbColor = Lit.OnLamp,
                uncheckedTrackColor = Lit.Raised,
                uncheckedThumbColor = Lit.Muted,
                uncheckedBorderColor = Color.Transparent,
            ),
        modifier = modifier.semantics { contentDescription = description },
    )
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
    Box(modifier.fillMaxWidth()) {
        SlabKey(
            Glyph.CheckDouble,
            "Complete ${entry.exerciseName}",
            onClick,
            modifier = Modifier.fillMaxWidth(),
            tone = if (entry.sets.all { it.completed }) KeyTone.Raised else KeyTone.Lamp,
        )
        // Centred, so a tap on these words lands on the key beneath them.
        Spoken("Complete Exercise", Modifier.align(Alignment.Center))
    }
}

/** Last time, at a glance: the numbers of each Set in a line, marked by the rewind symbol and its day. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun PreviousPerformanceStrip(previous: PreviousPerformance) {
    Row(
        Modifier.padding(start = 4.dp),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        GlyphIcon(Glyph.Rewind, tint = Lit.Muted, size = 18.dp, modifier = Modifier.padding(top = 1.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(formatShortDay(previous.date), style = Type.Small, color = Lit.Muted, modifier = Modifier.padding(top = 2.dp))
            Spoken("Last time · ${formatDay(previous.date)}")
            previous.sets.forEachIndexed { index, set ->
                Text(shortPerformance(set), style = Type.Body, color = Lit.Ink)
                Spoken(
                    listOfNotNull(
                        "Set ${index + 1}: ${describePerformance(set)}",
                        set.rpe?.let { "RPE $it" },
                    ).joinToString(separator = " · "),
                )
            }
        }
    }
}

/** A Set in the fewest marks: 8 ×, 30 s, +20. */
internal fun shortPerformance(set: com.patrickhaahr.openhome.fitness.WorkoutSet): String =
    listOfNotNull(
        set.reps?.let { "$it×" },
        set.durationSeconds?.let { "${it}s" },
        set.addedWeightKg?.let { "+${com.patrickhaahr.openhome.ui.plainNumber(it)}" },
    ).joinToString(" ")

/** A confirmed Set lights up. Nothing else on this screen is as bright, and nothing else needs saying. */
@Composable
private fun SetRow(
    exerciseName: String,
    set: ActiveSet,
    showMissingValue: Boolean,
    viewModel: ActiveWorkoutViewModel,
) {
    val problem = set.draft.check() as? SetCheck.Problem
    val label = "$exerciseName set ${set.number}"
    val fill by animateColorAsState(if (set.completed) Lit.Lamp else Lit.Slab, tween(Motion.PANEL), label = "set fill")
    val haptics = LocalHapticFeedback.current
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth().background(fill, SET_SHAPE).padding(6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                text = set.number.toString(),
                style = Type.Name,
                color = if (set.completed) Lit.OnLamp else Lit.Muted,
                textAlign = TextAlign.Center,
                modifier = Modifier.width(26.dp),
            )
            if (set.draft.measure.usesReps) {
                SetValueField(set, SetField.REPS, set.draft.reps, "$label reps", "×", KeyboardType.Number, viewModel)
            }
            if (set.draft.measure.usesSeconds) {
                SetValueField(set, SetField.DURATION, set.draft.durationSeconds, "$label seconds", "s", KeyboardType.Number, viewModel)
            }
            SetValueField(set, SetField.ADDED_WEIGHT, set.draft.addedWeightKg, "$label added kg", "kg", KeyboardType.Decimal, viewModel)
            SetValueField(set, SetField.RPE, set.draft.rpe, "$label RPE", "rpe", KeyboardType.Number, viewModel)
            val source = remember { MutableInteractionSource() }
            Box(
                Modifier
                    .size(CONFIRM)
                    .pressable(source)
                    .clip(CircleShape)
                    .background(if (set.completed) Lit.OnLamp else Lit.Raised)
                    .toggleable(
                        value = set.completed,
                        interactionSource = source,
                        indication = LocalIndication.current,
                        role = Role.Checkbox,
                        onValueChange = {
                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                            viewModel.setCompleted(set.id, it)
                        },
                    ).semantics { contentDescription = "$label performed" },
                contentAlignment = Alignment.Center,
            ) {
                GlyphIcon(Glyph.Check, tint = if (set.completed) Lit.Lamp else Lit.Muted, size = 26.dp)
            }
        }
        if (problem != null && (!problem.isMissingValue || showMissingValue)) {
            ProblemText(problem.message, Modifier.padding(start = 12.dp, bottom = 4.dp))
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
    unit: String,
    keyboard: KeyboardType,
    viewModel: ActiveWorkoutViewModel,
) {
    var text by rememberSaveable(set.id, field) { mutableStateOf(stored) }
    CompactValueField(
        value = text,
        onValueChange = {
            text = it
            viewModel.editSet(set.id, field, it)
        },
        description = description,
        unit = unit,
        keyboard = keyboard,
        highlighted = set.completed,
        modifier = Modifier.weight(1f),
    )
}

/** A figure in a well, with its unit tucked in the corner. Its description labels it the way a heading would. */
@Composable
internal fun CompactValueField(
    value: String,
    onValueChange: (String) -> Unit,
    description: String,
    unit: String,
    keyboard: KeyboardType,
    highlighted: Boolean,
    modifier: Modifier = Modifier,
) {
    val ink = if (highlighted) Lit.OnLamp else Lit.Ink
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        singleLine = true,
        textStyle = Type.Figure.copy(color = ink, textAlign = TextAlign.Center),
        cursorBrush = SolidColor(if (highlighted) Lit.OnLamp else Lit.Lamp),
        keyboardOptions = KeyboardOptions(keyboardType = keyboard),
        modifier =
            modifier
                .heightIn(min = CELL_HEIGHT)
                .background(if (highlighted) Lit.LampDeep else Lit.Raised, CELL_SHAPE),
        decorationBox = { inner ->
            Box(
                modifier =
                    Modifier
                        .heightIn(min = CELL_HEIGHT)
                        .padding(horizontal = 4.dp)
                        .semantics { contentDescription = description },
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    unit,
                    style = Type.Small,
                    color = if (highlighted) Lit.OnLamp else Lit.Muted,
                    modifier = Modifier.align(Alignment.TopCenter).padding(top = 7.dp),
                )
                Box(Modifier.padding(top = 14.dp), contentAlignment = Alignment.Center) {
                    if (value.isEmpty()) {
                        Text("–", style = Type.Figure, color = if (highlighted) Lit.OnLamp.copy(alpha = 0.45f) else Lit.Faint)
                    }
                    inner()
                }
            }
        },
    )
}

/** Finishing never silently records or drops a row: this shows what is kept, what is left out, and asks. */
@Composable
private fun FinishReview(workout: ActiveWorkout, onReturn: () -> Unit, onOmitAndFinish: () -> Unit) {
    val confirmed = workout.entries.sumOf { entry -> entry.sets.count { it.completed } }
    val unchecked =
        workout.entries.mapNotNull { entry ->
            entry.sets.filterNot { it.completed }.takeIf { it.isNotEmpty() }?.let { sets -> entry.exerciseName to sets.size }
        }
    KeyDialog(
        onDismiss = onReturn,
        title = if (confirmed == 0) "No confirmed Sets" else "Finish with unchecked Sets?",
        message =
            if (confirmed == 0) {
                "Check at least one performed Set to finish this Workout."
            } else {
                "${pluralize(confirmed, "confirmed Set")} will be recorded. These will be left out:"
            },
        glyph = Glyph.Flag,
        confirm = {
            if (confirmed > 0) GlyphKey(Glyph.Flag, "Omit and finish", onOmitAndFinish, tone = KeyTone.Lamp)
        },
        dismiss = { GlyphKey(Glyph.Back, "Return to logging", onReturn) },
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            GlyphIcon(Glyph.Check, tint = Lit.Lamp, size = 32.dp)
            Text(confirmed.toString(), style = Type.Display, color = Lit.Ink)
        }
        if (confirmed > 0) {
            unchecked.forEach { (name, count) ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    GlyphIcon(Glyph.Close, tint = Lit.Alarm, size = 20.dp)
                    Text(count.toString(), style = Type.Name, color = Lit.Ink)
                    Text(name, style = Type.Body, color = Lit.Muted)
                    Spoken("$name: ${pluralize(count, "unchecked Set")}")
                }
            }
        }
    }
}
