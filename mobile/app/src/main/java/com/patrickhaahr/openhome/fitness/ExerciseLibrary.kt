package com.patrickhaahr.openhome.fitness

import androidx.room.withTransaction
import com.patrickhaahr.openhome.storage.ExerciseEntity
import com.patrickhaahr.openhome.storage.OpenHomeDatabase
import com.patrickhaahr.openhome.storage.SyncProblem
import com.patrickhaahr.openhome.sync.FitnessOutbox
import com.patrickhaahr.openhome.sync.FitnessSyncScheduler
import com.patrickhaahr.openhome.sync.QueuedExercise

/** The Exercise Categories: descriptive tags that never limit where an Exercise may be used. */
val EXERCISE_CATEGORIES = listOf("calisthenics", "gym")

private const val MAX_NAME_LENGTH = 100

/** An Exercise as typed. Muscle group and equipment are optional, and blank means absent. */
data class ExerciseDraft(
    val name: String,
    val category: String?,
    val muscleGroup: String,
    val equipment: String,
) {
    fun check(): ExerciseCheck {
        val trimmedName = name.trim()
        return when {
            trimmedName.isEmpty() -> ExerciseCheck.NameProblem("Enter a name")
            trimmedName.length > MAX_NAME_LENGTH -> ExerciseCheck.NameProblem("Keep the name to $MAX_NAME_LENGTH characters")
            category !in EXERCISE_CATEGORIES -> ExerciseCheck.CategoryProblem
            else ->
                ExerciseCheck.Valid(
                    QueuedExercise(
                        name = trimmedName,
                        category = checkNotNull(category),
                        muscleGroup = muscleGroup.trim().ifEmpty { null },
                        equipment = equipment.trim().ifEmpty { null },
                    ),
                )
        }
    }
}

sealed interface ExerciseCheck {
    data class Valid(val exercise: QueuedExercise) : ExerciseCheck

    data class NameProblem(val message: String) : ExerciseCheck

    data object CategoryProblem : ExerciseCheck
}

sealed interface ExerciseSave {
    data class Saved(val exerciseId: Long) : ExerciseSave

    data class Invalid(val problem: ExerciseCheck) : ExerciseSave

    /** The Exercise left the refused state meanwhile, so there is nothing to correct any more. */
    data object NoLongerRefused : ExerciseSave
}

/** An Exercise waiting for the user, with what Axum said about it. */
data class RefusedExercise(val exercise: Exercise, val problem: SyncProblem)

/** What the Library shows: all of [query], and the chosen tags. Null means that tag is not filtering. */
data class ExerciseFilter(
    val query: String = "",
    val category: String? = null,
    val muscleGroup: String? = null,
) {
    val isActive: Boolean
        get() = query.isNotBlank() || category != null || muscleGroup != null

    fun matches(exercise: Exercise): Boolean {
        val text = query.trim()
        val inText =
            listOfNotNull(exercise.name, exercise.muscleGroup, exercise.equipment)
                .any { it.contains(text, ignoreCase = true) }
        return inText &&
            (category == null || exercise.category == category) &&
            (muscleGroup == null || exercise.muscleGroup?.trim().equals(muscleGroup, ignoreCase = true))
    }
}

/** The muscle groups the Library actually uses, however their case was typed. */
fun List<Exercise>.muscleGroups(): List<String> =
    mapNotNull { it.muscleGroup?.trim()?.takeIf(String::isNotEmpty) }
        .distinctBy { it.lowercase() }
        .sortedBy { it.lowercase() }

/**
 * Exercises created on this device. Each commits together with its queued sync, and is in the Library
 * and the workout picker from that moment, whatever the network does.
 */
class ExerciseLibrary(
    private val database: OpenHomeDatabase,
    private val outbox: FitnessOutbox,
    private val scheduler: FitnessSyncScheduler,
) {
    private val dao = database.fitnessDao()

    suspend fun create(draft: ExerciseDraft): ExerciseSave {
        val saved =
            database.withTransaction {
                val check = checkAgainstLibrary(draft, exceptId = NO_EXERCISE)
                if (check !is ExerciseCheck.Valid) return@withTransaction ExerciseSave.Invalid(check)
                val exercise = check.exercise
                val id =
                    dao.insertExercise(
                        ExerciseEntity(
                            serverId = null,
                            name = exercise.name,
                            category = exercise.category,
                            muscleGroup = exercise.muscleGroup,
                            equipment = exercise.equipment,
                        ),
                    )
                outbox.enqueueExercise(id, exercise)
                ExerciseSave.Saved(id)
            }
        // After the commit: if the process dies first, the next app start schedules it from the outbox.
        if (saved is ExerciseSave.Saved) scheduler.schedule()
        return saved
    }

    suspend fun loadRefused(exerciseId: Long): RefusedExercise? {
        val problem = outbox.refusalOfExercise(exerciseId) ?: return null
        val row = dao.loadExercise(exerciseId) ?: return null
        return RefusedExercise(Exercise(row.id, row.name, row.category, row.muscleGroup, row.equipment), problem)
    }

    /** Corrects a refused Exercise everywhere it is used, and sends it again ahead of its Workouts. */
    suspend fun correctRefused(exerciseId: Long, draft: ExerciseDraft): ExerciseSave {
        val saved =
            database.withTransaction {
                val check = checkAgainstLibrary(draft, exceptId = exerciseId)
                if (check !is ExerciseCheck.Valid) return@withTransaction ExerciseSave.Invalid(check)
                val exercise = check.exercise
                if (!outbox.replaceRefusedExercise(exerciseId, exercise)) {
                    return@withTransaction ExerciseSave.NoLongerRefused
                }
                dao.updateExerciseDetails(exerciseId, exercise.name, exercise.category, exercise.muscleGroup, exercise.equipment)
                ExerciseSave.Saved(exerciseId)
            }
        if (saved is ExerciseSave.Saved) scheduler.schedule()
        return saved
    }

    /** The user's answer to a taken name: this is that Exercise, so link to it rather than create one. */
    suspend fun useExistingOnServer(exerciseId: Long): ExerciseSave {
        val saved =
            database.withTransaction {
                val row = dao.loadExercise(exerciseId) ?: return@withTransaction ExerciseSave.NoLongerRefused
                val queued =
                    QueuedExercise(row.name, row.category, row.muscleGroup, row.equipment, useExisting = true)
                if (outbox.replaceRefusedExercise(exerciseId, queued)) {
                    ExerciseSave.Saved(exerciseId)
                } else {
                    ExerciseSave.NoLongerRefused
                }
            }
        if (saved is ExerciseSave.Saved) scheduler.schedule()
        return saved
    }

    /** Two names differing only in case would read as one Exercise here, so they are refused. */
    private suspend fun checkAgainstLibrary(draft: ExerciseDraft, exceptId: Long): ExerciseCheck {
        val check = draft.check()
        if (check !is ExerciseCheck.Valid) return check
        val taken = dao.exerciseNamesExcept(exceptId).any { it.equals(check.exercise.name, ignoreCase = true) }
        return if (taken) ExerciseCheck.NameProblem("${check.exercise.name} is already in your Library") else check
    }

    private companion object {
        const val NO_EXERCISE = -1L
    }
}
