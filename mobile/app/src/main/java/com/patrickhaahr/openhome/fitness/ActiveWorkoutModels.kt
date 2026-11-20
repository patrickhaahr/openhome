package com.patrickhaahr.openhome.fitness

import com.patrickhaahr.openhome.storage.SetMeasure
import kotlinx.serialization.Serializable

private const val MAX_RPE = 10L

data class ActiveWorkout(
    val date: String,
    val name: String,
    val notes: String,
    val entries: List<ActiveEntry>,
    private val currentEntryId: Long?,
) {
    val currentEntry: ActiveEntry?
        get() = entries.firstOrNull { it.id == currentEntryId } ?: entries.firstOrNull()
}

data class ActiveEntry(
    val id: Long,
    val exerciseId: Long,
    val exerciseName: String,
    val exerciseCategory: String,
    val sets: List<ActiveSet>,
    val rest: RestPreference,
)

data class ActiveSet(val id: Long, val number: Long, val draft: SetDraft, val completed: Boolean)

enum class SetField {
    REPS,
    ADDED_WEIGHT,
    DURATION,
    RPE,
}

@Serializable
data class SetDraft(
    val measure: SetMeasure,
    val reps: String,
    val addedWeightKg: String,
    val durationSeconds: String,
    val rpe: String,
) {
    fun check(): SetCheck {
        val repsCheck = if (measure.usesReps) amount(reps, "reps", "Reps") else AmountCheck.Valid(null)
        if (repsCheck is AmountCheck.Problem) return repsCheck.problem
        val secondsCheck =
            if (measure.usesSeconds) amount(durationSeconds, "seconds", "Seconds") else AmountCheck.Valid(null)
        if (secondsCheck is AmountCheck.Problem) return secondsCheck.problem
        val weight =
            addedWeightKg.trim().replace(',', '.').let { text ->
                if (text.isEmpty()) {
                    null
                } else {
                    text.toDoubleOrNull()?.takeIf { it.isFinite() && it > 0 }
                        ?: return SetCheck.Problem("Added Weight must be above 0, or blank for bodyweight")
                }
            }
        val rpeValue =
            rpe.trim().let { text ->
                if (text.isEmpty()) null else text.toLongOrNull()?.takeIf { it in 1..MAX_RPE } ?: return SetCheck.Problem("RPE must be 1 to 10")
            }
        return SetCheck.Valid(
            reps = (repsCheck as AmountCheck.Valid).value,
            durationSeconds = (secondsCheck as AmountCheck.Valid).value,
            addedWeightKg = weight,
            rpe = rpeValue,
        )
    }

    private fun amount(text: String, label: String, unit: String): AmountCheck {
        val trimmed = text.trim()
        if (trimmed.isEmpty()) return AmountCheck.Problem(SetCheck.Problem("Enter $label to confirm", isMissingValue = true))
        val value =
            trimmed.toLongOrNull()?.takeIf { it > 0 }
                ?: return AmountCheck.Problem(SetCheck.Problem("$unit must be a whole number above 0"))
        return AmountCheck.Valid(value)
    }

    private sealed interface AmountCheck {
        data class Valid(val value: Long?) : AmountCheck

        data class Problem(val problem: SetCheck.Problem) : AmountCheck
    }
}

sealed interface SetCheck {
    data class Valid(
        val reps: Long?,
        val durationSeconds: Long?,
        val addedWeightKg: Double?,
        val rpe: Long?,
    ) : SetCheck

    data class Problem(val message: String, val isMissingValue: Boolean = false) : SetCheck
}

data class ExerciseCompletion(val confirmed: Int, val refused: Set<Long>)

sealed interface FinishOutcome {
    data class Finished(val workoutId: Long) : FinishOutcome

    data object HasUncheckedSets : FinishOutcome

    data object NothingConfirmed : FinishOutcome

    data object NoActiveWorkout : FinishOutcome
}
