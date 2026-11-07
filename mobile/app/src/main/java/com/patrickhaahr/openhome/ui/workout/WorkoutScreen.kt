package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.absoluteOffset
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
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
import com.patrickhaahr.openhome.fitness.ActiveWorkout
import com.patrickhaahr.openhome.fitness.ActiveWorkoutRepository
import com.patrickhaahr.openhome.fitness.FitnessOverview
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.fitness.RefusedExercise
import com.patrickhaahr.openhome.fitness.WorkoutSummary
import com.patrickhaahr.openhome.sync.FitnessSyncStatus
import com.patrickhaahr.openhome.sync.SyncSnapshot
import com.patrickhaahr.openhome.sync.SyncStatus
import com.patrickhaahr.openhome.ui.ProfileKey
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.SlabShape
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.WithSpoken
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.LitPlate
import com.patrickhaahr.openhome.ui.design.OpenHomeMark
import com.patrickhaahr.openhome.ui.design.SegmentRing
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.formatShortDay
import com.patrickhaahr.openhome.ui.formatWeekday
import com.patrickhaahr.openhome.ui.pluralize
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import java.time.LocalDate
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

private const val PLATE_TO_WIDTH = 1.24f
private val PLATE_TOP = 36.dp
private val PLATE_LEFT_MARGIN = 14.dp
private val BENCH_HEIGHT = 104.dp

class WorkoutViewModel(
    fitness: FitnessRepository,
    private val activeWorkouts: ActiveWorkoutRepository,
    private val syncStatus: FitnessSyncStatus,
    private val saveScope: CoroutineScope,
) : ViewModel() {
    val overview: StateFlow<FitnessOverview?> =
        fitness.observeOverview().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    val activeWorkout: StateFlow<ActiveLookup> =
        activeWorkouts.observe()
            .map<ActiveWorkout?, ActiveLookup> { ActiveLookup.Loaded(it) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), ActiveLookup.Loading)

    val sync: StateFlow<SyncStatus?> =
        syncStatus.observe()
            .map<SyncSnapshot, SyncStatus?> { it.overall }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    /** What only the user can settle; everything queued behind these waits for them. */
    val refusedExercises: StateFlow<List<RefusedExercise>> =
        combine(fitness.observeExercises(), syncStatus.observe()) { exercises, snapshot ->
            exercises.mapNotNull { exercise ->
                snapshot.exerciseRefusal(exercise.id)?.let { RefusedExercise(exercise, it) }
            }
        }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val profileNeedsTheUser: StateFlow<Boolean> =
        syncStatus.observe()
            .map { it.profileNeedsTheUser }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), false)

    fun startEmpty(onStarted: () -> Unit) {
        saveScope.launch {
            activeWorkouts.startEmpty(LocalDate.now().toString())
            onStarted()
        }
    }

    fun repeat(workout: WorkoutSummary, onStarted: () -> Unit) {
        saveScope.launch {
            if (activeWorkouts.repeat(workout.id, LocalDate.now().toString())) onStarted()
        }
    }

    fun syncNow() {
        saveScope.launch { syncStatus.syncNow() }
    }
}

@Composable
fun WorkoutRoute(
    onOpenHistory: () -> Unit,
    onOpenLibrary: () -> Unit,
    onFixExercise: (Long) -> Unit,
    onOpenProfile: () -> Unit,
    onOpenActiveWorkout: () -> Unit,
) {
    val viewModel =
        appViewModel { WorkoutViewModel(it.fitness, it.activeWorkout, it.syncStatus, it.saveScope) }
    val overview by viewModel.overview.collectAsStateWithLifecycle()
    val active by viewModel.activeWorkout.collectAsStateWithLifecycle()
    val sync by viewModel.sync.collectAsStateWithLifecycle()
    val refusedExercises by viewModel.refusedExercises.collectAsStateWithLifecycle()
    val profileNeedsTheUser by viewModel.profileNeedsTheUser.collectAsStateWithLifecycle()
    val workout = (active as? ActiveLookup.Loaded)?.workout
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Box(Modifier.fillMaxWidth()) {
            // Waits for storage, so Start Empty is never shown as primary by mistake.
            when {
                active == ActiveLookup.Loading -> Unit
                workout != null -> PlateStage { ResumePlate(workout, onOpenActiveWorkout, it) }
                else ->
                    overview?.let { loaded ->
                        PlateStage {
                            StartPlate(
                                latest = loaded.latestWorkout,
                                onRepeat = { latest -> viewModel.repeat(latest, onOpenActiveWorkout) },
                                onStartEmpty = { viewModel.startEmpty(onOpenActiveWorkout) },
                                modifier = it,
                            )
                        }
                    }
            }
            TopMarks(sync, viewModel::syncNow, onOpenProfile)
        }
        Column(
            modifier = Modifier.fillMaxWidth().padding(horizontal = ScreenMargin),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            refusedExercises.forEach { row ->
                RefusedCard(
                    title = row.exercise.name,
                    message = exerciseRefusalMessage(row.problem),
                    description = "Fix Exercise",
                    onClick = { onFixExercise(row.exercise.id) },
                )
            }
            if (profileNeedsTheUser) {
                RefusedCard(
                    title = "Profile and Body Weight",
                    message = "OpenHome refused a change. Tap to review it.",
                    description = "Open Profile",
                    onClick = onOpenProfile,
                )
            }
            overview?.let { loaded ->
                LastWorkout(loaded.latestWorkout)
                Bench(
                    overview = loaded,
                    canStartEmpty = workout == null && active != ActiveLookup.Loading && loaded.latestWorkout != null,
                    onStartEmpty = { viewModel.startEmpty(onOpenActiveWorkout) },
                    onOpenHistory = onOpenHistory,
                    onOpenLibrary = onOpenLibrary,
                )
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun TopMarks(sync: SyncStatus?, onSyncNow: () -> Unit, onOpenProfile: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(start = ScreenMargin + 4.dp, end = ScreenMargin, top = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        OpenHomeMark(Modifier.size(32.dp))
        // In the corner the plate's curve leaves free, so it never sits on the casting.
        Box(Modifier.weight(1f)) { sync?.let { SyncStatusLine(it, onSyncNow = onSyncNow) } }
        Box(Modifier.background(Lit.Ground, CircleShape)) { ProfileKey(onOpenProfile) }
    }
}

/** The plate is larger than the phone: it hangs off the right edge, the way the real thing fills a bar. */
@Composable
private fun PlateStage(plate: @Composable (Modifier) -> Unit) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val diameter = maxWidth * PLATE_TO_WIDTH
        // A larger child is laid out centred on its parent, so this is measured from there.
        val overhang = (diameter - maxWidth) / 2
        Box(Modifier.fillMaxWidth().height(diameter + PLATE_TOP).padding(top = PLATE_TOP)) {
            plate(
                Modifier
                    .absoluteOffset(x = overhang + PLATE_LEFT_MARGIN)
                    .requiredSize(diameter),
            )
        }
    }
}

/** Repeating is the usual session, so it is the plate itself; with no History, it simply starts empty. */
@Composable
private fun StartPlate(
    latest: WorkoutSummary?,
    onRepeat: (WorkoutSummary) -> Unit,
    onStartEmpty: () -> Unit,
    modifier: Modifier,
) {
    if (latest == null) {
        LitPlate(
            Glyph.Plus,
            engraved = null,
            modifier = modifier.key("Start Empty", onStartEmpty, depth = 0.97f),
        )
    } else {
        LitPlate(
            Glyph.Repeat,
            engraved = latest.exerciseNames?.engraving(),
            modifier = modifier.key("Repeat Last Workout", { onRepeat(latest) }, depth = 0.97f),
        )
    }
}

private fun String.engraving(): String = "${split(" · ").joinToString(" · ").uppercase()} · "

/** An unfinished Workout turns the plate dark: it lights one segment for every Set confirmed. */
@Composable
private fun ResumePlate(workout: ActiveWorkout, onClick: () -> Unit, modifier: Modifier) {
    val sets = workout.entries.flatMap { it.sets }
    val confirmed = sets.count { it.completed }
    Box(modifier.key("Resume Workout", onClick, depth = 0.97f).background(Lit.Slab, CircleShape), contentAlignment = Alignment.Center) {
        SegmentRing(confirmed, sets.size, Modifier.fillMaxSize().padding(14.dp), thickness = 36.dp)
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            GlyphIcon(Glyph.Play, tint = Lit.Lamp, size = 56.dp)
            Text("$confirmed/${sets.size}", style = Type.Display, color = Lit.Ink, modifier = Modifier.padding(top = 12.dp))
            Text(formatShortDay(workout.date), style = Type.Name, color = Lit.Muted)
        }
    }
    Spoken("${formatDay(workout.date)} · ${pluralize(confirmed, "Set")} confirmed")
}

/** What the plate would repeat: when it was, what it was called and how much of it there was. */
@Composable
private fun LastWorkout(latest: WorkoutSummary?) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 4.dp)) {
        Spoken("Last Workout")
        if (latest == null) {
            Spoken("No Workouts yet")
            Spoken("Workouts you log will appear here.")
        } else {
            Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(formatShortDay(latest.date), style = Type.Display, color = Lit.Ink)
                Text(
                    formatWeekday(latest.date),
                    style = Type.Name,
                    color = Lit.Muted,
                    modifier = Modifier.padding(bottom = 6.dp),
                )
                Spacer(Modifier.weight(1f))
                GlyphIcon(Glyph.Check, tint = Lit.Lamp, size = 22.dp, modifier = Modifier.padding(bottom = 8.dp))
                Text(latest.setCount.toString(), style = Type.Title, color = Lit.Ink)
            }
            Spoken(formatDay(latest.date))
            latest.name?.let {
                Text(it, style = Type.Name, color = Lit.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            latest.exerciseNames?.let { Spoken(it) }
        }
    }
}

/** The three things beside the plate: an empty bar, the log of everything lifted, and the shelf of Exercises. */
@Composable
private fun Bench(
    overview: FitnessOverview,
    canStartEmpty: Boolean,
    onStartEmpty: () -> Unit,
    onOpenHistory: () -> Unit,
    onOpenLibrary: () -> Unit,
) {
    WithSpoken(
        spoken = {
            Spoken("${pluralize(overview.workoutCount, "Workout")} on this device")
            Spoken("${pluralize(overview.exerciseCount, "Exercise")} in your Library")
        },
    ) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        if (canStartEmpty) {
            GlyphKey(
                Glyph.Plus,
                "Start Empty",
                onStartEmpty,
                modifier = Modifier.height(BENCH_HEIGHT).weight(0.72f),
                shape = SlabShape,
                tone = KeyTone.Slab,
                size = BENCH_HEIGHT,
                glyphSize = 32.dp,
            )
        }
        BenchKey(Glyph.Rewind, "History", overview.workoutCount, onOpenHistory, Modifier.weight(1f))
        BenchKey(Glyph.Stack, "Library", overview.exerciseCount, onOpenLibrary, Modifier.weight(1f))
    }
    }
}

@Composable
private fun BenchKey(glyph: Glyph, description: String, figure: Int, onClick: () -> Unit, modifier: Modifier) {
    Column(
        modifier
            .height(BENCH_HEIGHT)
            .key(description, onClick, shape = SlabShape)
            .background(Lit.Slab, SlabShape)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.SpaceBetween,
    ) {
        GlyphIcon(glyph, tint = Lit.Lamp, size = 26.dp)
        Text(figure.toString(), style = Type.Title, color = Lit.Ink)
    }
}

/** What only the user can settle is the one place this screen uses a sentence. */
@Composable
private fun RefusedCard(title: String, message: String, description: String, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .key(description, onClick, shape = SlabShape, depth = 0.98f)
            .background(Lit.AlarmDeep, SlabShape)
            .padding(horizontal = 20.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        GlyphIcon(Glyph.Warn, tint = Lit.Alarm, size = 26.dp)
        Column(Modifier.weight(1f)) {
            Text(text = title, style = Type.Name, color = Lit.Ink)
            Text(text = message, style = Type.Body, color = Lit.Alarm)
        }
        GlyphIcon(Glyph.Right, tint = Lit.Muted, size = 20.dp)
    }
}
