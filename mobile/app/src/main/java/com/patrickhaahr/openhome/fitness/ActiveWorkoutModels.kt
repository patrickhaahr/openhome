package com.patrickhaahr.openhome.fitness

import com.patrickhaahr.openhome.storage.SetMeasure

data class ActiveWorkout(
    val date: String,
    val name: String,
    val notes: String,
    val entries: List<ActiveEntry>,
)

data class ActiveEntry(
    val id: Long,
    val exerciseName: String,
    val exerciseCategory: String,
    val sets: List<ActiveSet>,
)

data class ActiveSet(val id: Long, val number: Long, val draft: SetDraft, val completed: Boolean)

enum class SetField {
    REPS,
    ADDED_WEIGHT,
    DURATION,
    RPE,
}

/** A row's values exactly as typed. The field its [measure] does not use is not part of the Set. */
data class SetDraft(
    val measure: SetMeasure,
    val reps: String,
    val addedWeightKg: String,
    val durationSeconds: String,
    val rpe: String,
) {
    fun check(): SetCheck {
        val amount = amount().let { if (it is AmountCheck.Problem) return it.problem else it as AmountCheck.Valid }
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
        return when (measure) {
            SetMeasure.REPS -> SetCheck.Valid(reps = amount.value, durationSeconds = null, weight, rpeValue)
            SetMeasure.HOLD -> SetCheck.Valid(reps = null, durationSeconds = amount.value, weight, rpeValue)
        }
    }

    private fun amount(): AmountCheck {
        val (text, label, unit) =
            when (measure) {
                SetMeasure.REPS -> Triple(reps, "reps", "Reps")
                SetMeasure.HOLD -> Triple(durationSeconds, "seconds", "Seconds")
            }
        val trimmed = text.trim()
        if (trimmed.isEmpty()) return AmountCheck.Problem(SetCheck.Problem("Enter $label to confirm", isMissingValue = true))
        val value =
            trimmed.toLongOrNull()?.takeIf { it > 0 }
                ?: return AmountCheck.Problem(SetCheck.Problem("$unit must be a whole number above 0"))
        return AmountCheck.Valid(value)
    }

    private sealed interface AmountCheck {
        data class Valid(val value: Long) : AmountCheck

        data class Problem(val problem: SetCheck.Problem) : AmountCheck
    }

    private companion object {
        const val MAX_RPE = 10L
    }
}

/** Only a [Valid] row can be a performed Set. Null keeps its domain meaning; it is never zero. */
sealed interface SetCheck {
    data class Valid(
        val reps: Long?,
        val durationSeconds: Long?,
        val addedWeightKg: Double?,
        val rpe: Long?,
    ) : SetCheck

    /** [isMissingValue] marks a row not yet filled in, which is only worth mentioning on confirming. */
    data class Problem(val message: String, val isMissingValue: Boolean = false) : SetCheck
}

sealed interface FinishOutcome {
    data class Finished(val workoutId: Long) : FinishOutcome

    /** Nothing was recorded: the caller has not chosen to omit the unchecked rows. */
    data object HasUncheckedSets : FinishOutcome

    /** Nothing was recorded: a Workout needs at least one confirmed Set. */
    data object NothingConfirmed : FinishOutcome

    data object NoActiveWorkout : FinishOutcome
}
