package com.patrickhaahr.openhome.ui.workout

import android.database.SQLException
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.AlertDialog
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.ActiveWorkout
import com.patrickhaahr.openhome.fitness.ActiveWorkoutRepository
import com.patrickhaahr.openhome.fitness.FinishOutcome
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.fitness.PreviousPerformance
import com.patrickhaahr.openhome.fitness.RestTimers
import com.patrickhaahr.openhome.fitness.SetField
import com.patrickhaahr.openhome.storage.SetMeasure
import com.patrickhaahr.openhome.ui.DayPicker
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.theme.Lit
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


sealed interface ActiveLookup {
    data object Loading : ActiveLookup

    data class Loaded(val workout: ActiveWorkout?) : ActiveLookup
}

internal enum class SaveState {
    SAVING,
    SAVED,
    FAILED,
}

internal class ActiveWorkoutViewModel(
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

internal fun ActiveWorkout.hasOnlyConfirmedSets(): Boolean =
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
