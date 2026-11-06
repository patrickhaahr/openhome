package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.ActiveWorkoutRepository
import com.patrickhaahr.openhome.fitness.EXERCISE_CATEGORIES
import com.patrickhaahr.openhome.fitness.ExerciseCheck
import com.patrickhaahr.openhome.fitness.ExerciseDraft
import com.patrickhaahr.openhome.fitness.ExerciseLibrary
import com.patrickhaahr.openhome.fitness.ExerciseSave
import com.patrickhaahr.openhome.fitness.RefusedExercise
import com.patrickhaahr.openhome.storage.SyncProblem
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.FormField
import com.patrickhaahr.openhome.ui.ProblemText
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.appViewModel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

private data class FormProblems(
    val name: String? = null,
    val category: String? = null,
    val general: String? = null,
)

/** Saves in the app's scope: once the user has pressed Save, leaving the screen cannot lose it. */
private class ExerciseFormViewModel(
    private val library: ExerciseLibrary,
    private val activeWorkouts: ActiveWorkoutRepository,
    private val saveScope: CoroutineScope,
) : ViewModel() {
    private val problemsNow = MutableStateFlow(FormProblems())
    val problems: StateFlow<FormProblems> = problemsNow

    private val refusedNow = MutableStateFlow<RefusedLookup>(RefusedLookup.Loading)
    val refused: StateFlow<RefusedLookup> = refusedNow

    private var saving = false

    fun loadRefused(exerciseId: Long) {
        viewModelScope.launch { refusedNow.value = RefusedLookup.Loaded(library.loadRefused(exerciseId)) }
    }

    fun create(draft: ExerciseDraft, addToWorkout: Boolean, onSaved: () -> Unit) =
        save(onSaved) {
            library.create(draft).also { saved ->
                if (addToWorkout && saved is ExerciseSave.Saved) activeWorkouts.addExercise(saved.exerciseId)
            }
        }

    fun correct(exerciseId: Long, draft: ExerciseDraft, onSaved: () -> Unit) =
        save(onSaved) { library.correctRefused(exerciseId, draft) }

    fun useExisting(exerciseId: Long, onSaved: () -> Unit) =
        save(onSaved) { library.useExistingOnServer(exerciseId) }

    private fun save(onSaved: () -> Unit, change: suspend () -> ExerciseSave) {
        if (saving) return
        saving = true
        saveScope.launch {
            val outcome =
                runCatching { change() }
                    .getOrElse {
                        problemsNow.value = FormProblems(general = "Couldn't save on this device. Please try again.")
                        saving = false
                        return@launch
                    }
            saving = false
            when (outcome) {
                is ExerciseSave.Saved -> onSaved()
                is ExerciseSave.Invalid ->
                    problemsNow.value =
                        when (val problem = outcome.problem) {
                            is ExerciseCheck.NameProblem -> FormProblems(name = problem.message)
                            ExerciseCheck.CategoryProblem -> FormProblems(category = "Choose a category")
                            is ExerciseCheck.Valid -> FormProblems()
                        }
                ExerciseSave.NoLongerRefused ->
                    problemsNow.value = FormProblems(general = "This Exercise is already syncing again.")
            }
        }
    }
}

private sealed interface RefusedLookup {
    data object Loading : RefusedLookup

    data class Loaded(val refused: RefusedExercise?) : RefusedLookup
}

/** Works without a connection: the Exercise is usable as soon as it is saved on the device. */
@Composable
fun NewExerciseRoute(initialName: String, addToWorkout: Boolean, onSaved: () -> Unit) {
    val viewModel = exerciseFormViewModel()
    val problems by viewModel.problems.collectAsStateWithLifecycle()
    ExerciseForm(
        title = "New Exercise",
        initial = ExerciseDraft(initialName, category = null, muscleGroup = "", equipment = ""),
        problems = problems,
        saveLabel = if (addToWorkout) "Save and add to Workout" else "Save Exercise",
        onSave = { viewModel.create(it, addToWorkout, onSaved) },
    )
}

/** Correcting what Axum refused. Workouts already logged with this Exercise simply follow the fix. */
@Composable
fun FixExerciseRoute(exerciseId: Long, onSaved: () -> Unit) {
    val viewModel = exerciseFormViewModel()
    val problems by viewModel.problems.collectAsStateWithLifecycle()
    val lookup by viewModel.refused.collectAsStateWithLifecycle()
    LaunchedEffect(exerciseId) { viewModel.loadRefused(exerciseId) }
    when (val current = lookup) {
        RefusedLookup.Loading -> Unit
        is RefusedLookup.Loaded -> {
            val refused = current.refused
            if (refused == null) {
                EmptyState(title = "Nothing to fix", message = "This Exercise is no longer refused.")
            } else {
                ExerciseForm(
                    title = "Fix Exercise",
                    initial =
                        ExerciseDraft(
                            name = refused.exercise.name,
                            category = refused.exercise.category,
                            muscleGroup = refused.exercise.muscleGroup.orEmpty(),
                            equipment = refused.exercise.equipment.orEmpty(),
                        ),
                    problems = problems,
                    saveLabel = "Save and sync again",
                    explanation = refusalExplanation(refused),
                    onSave = { viewModel.correct(exerciseId, it, onSaved) },
                    onUseExisting =
                        if (refused.problem == SyncProblem.NAME_TAKEN) {
                            { viewModel.useExisting(exerciseId, onSaved) }
                        } else {
                            null
                        },
                )
            }
        }
    }
}

@Composable
private fun exerciseFormViewModel(): ExerciseFormViewModel =
    appViewModel { ExerciseFormViewModel(it.exerciseLibrary, it.activeWorkout, it.saveScope) }

private fun refusalExplanation(refused: RefusedExercise): String =
    when (refused.problem) {
        SyncProblem.NAME_TAKEN ->
            "OpenHome already has an Exercise named ${refused.exercise.name}. Rename this one, or use " +
                "OpenHome's. Workouts logged with it stay on this device and sync afterwards."
        else ->
            "OpenHome didn't accept this Exercise. Check its details and save again. Workouts logged " +
                "with it stay on this device and sync afterwards."
    }

@Composable
private fun ExerciseForm(
    title: String,
    initial: ExerciseDraft,
    problems: FormProblems,
    saveLabel: String,
    onSave: (ExerciseDraft) -> Unit,
    explanation: String? = null,
    onUseExisting: (() -> Unit)? = null,
) {
    var name by rememberSaveable { mutableStateOf(initial.name) }
    var category by rememberSaveable { mutableStateOf(initial.category) }
    var muscleGroup by rememberSaveable { mutableStateOf(initial.muscleGroup) }
    var equipment by rememberSaveable { mutableStateOf(initial.equipment) }

    Column(Modifier.fillMaxSize().imePadding().verticalScroll(rememberScrollState())) {
        ScreenHeader(title = title)
        Column(
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            explanation?.let { Text(text = it, style = MaterialTheme.typography.bodyMedium) }
            FormField(
                value = name,
                onValueChange = { name = it },
                label = "Name",
                problem = problems.name,
                capitalization = KeyboardCapitalization.Words,
            )
            CategoryField(selected = category, problem = problems.category, onSelect = { category = it })
            FormField(value = muscleGroup, onValueChange = { muscleGroup = it }, label = "Muscle group (optional)")
            FormField(value = equipment, onValueChange = { equipment = it }, label = "Equipment (optional)")
            problems.general?.let { ProblemText(it) }
            Button(
                onClick = { onSave(ExerciseDraft(name, category, muscleGroup, equipment)) },
                modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp),
            ) {
                Text(saveLabel)
            }
            onUseExisting?.let {
                OutlinedButton(onClick = it, modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp)) {
                    Text("Use OpenHome's Exercise")
                }
            }
            Text(
                text = "Saved on this device first, then synced over Wi-Fi.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(bottom = 16.dp),
            )
        }
    }
}

@Composable
private fun CategoryField(selected: String?, problem: String?, onSelect: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(
            text = "Category",
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            EXERCISE_CATEGORIES.forEach { category ->
                FilterChip(
                    selected = selected == category,
                    onClick = { onSelect(category) },
                    label = { Text(categoryLabel(category)) },
                )
            }
        }
        problem?.let { ProblemText(it) }
    }
}
