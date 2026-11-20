package com.patrickhaahr.openhome.fitness

import androidx.room.withTransaction
import com.patrickhaahr.openhome.storage.BodyWeightEntity
import com.patrickhaahr.openhome.storage.OpenHomeDatabase
import com.patrickhaahr.openhome.storage.ProfileEntity
import com.patrickhaahr.openhome.sync.FitnessOutbox
import com.patrickhaahr.openhome.sync.FitnessSyncScheduler
import com.patrickhaahr.openhome.sync.QueuedBodyWeight
import com.patrickhaahr.openhome.sync.QueuedProfile
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeParseException

val PROFILE_SEXES = listOf("male", "female")

private val MAX_HEART_RATE_RANGE = 100L..240L
private val HOME_TIME_ZONE: ZoneId = ZoneId.of("Europe/Copenhagen")

val EARLIEST_BIRTH_DATE: LocalDate = LocalDate.of(1900, 1, 1)

fun latestBirthDate(): LocalDate = LocalDate.now(HOME_TIME_ZONE)

data class ProfileDraft(
    val heightCm: String,
    val sex: String?,
    val birthDate: String?,
    val maxHeartRate: String,
) {
    fun check(): ProfileCheck {
        val height = heightCm.toOptionalDecimal()?.takeIf { it.isFinite() && it > 0 }
        val heartRate = maxHeartRate.trim().toLongOrNull()?.takeIf { it in MAX_HEART_RATE_RANGE }
        val heightProblem = heightCm.isNotBlank() && height == null
        val heartRateProblem = maxHeartRate.isNotBlank() && heartRate == null
        val birthDateProblem = birthDate != null && !isValidBirthDate(birthDate)
        return if (heightProblem || heartRateProblem || birthDateProblem) {
            ProfileCheck.Problems(
                height = "Enter height in cm above 0, or leave it blank".takeIf { heightProblem },
                maxHeartRate = "Enter a whole number from 100 to 240, or leave it blank".takeIf { heartRateProblem },
                birthDate = "Choose a date from 1900 up to today".takeIf { birthDateProblem },
            )
        } else {
            ProfileCheck.Valid(Profile(height, sex, birthDate, heartRate))
        }
    }
}

private fun isValidBirthDate(text: String): Boolean =
    try {
        LocalDate.parse(text) in EARLIEST_BIRTH_DATE..latestBirthDate()
    } catch (_: DateTimeParseException) {
        false
    }

private fun String.toOptionalDecimal(): Double? = trim().replace(',', '.').toDoubleOrNull()

sealed interface ProfileCheck {
    data class Valid(val profile: Profile) : ProfileCheck

    data class Problems(val height: String?, val maxHeartRate: String?, val birthDate: String?) : ProfileCheck
}

sealed interface ProfileSave {
    data object Saved : ProfileSave

    data class Invalid(val problems: ProfileCheck.Problems) : ProfileSave
}

sealed interface BodyWeightSave {
    data object Saved : BodyWeightSave

    data object InvalidWeight : BodyWeightSave

    data class AlreadyRecorded(val existing: BodyWeight) : BodyWeightSave
}

class ProfileRecords(
    private val database: OpenHomeDatabase,
    private val outbox: FitnessOutbox,
    private val scheduler: FitnessSyncScheduler,
) {
    private val dao = database.fitnessDao()

    suspend fun loadProfile(): Profile? = dao.loadProfile()?.toDomain()

    suspend fun saveProfile(draft: ProfileDraft): ProfileSave {
        val check = draft.check()
        if (check is ProfileCheck.Problems) return ProfileSave.Invalid(check)
        val profile = (check as ProfileCheck.Valid).profile
        val queued =
            database.withTransaction {
                val stored = dao.loadProfile()
                if (stored?.toDomain() == profile) return@withTransaction false
                val revision = (stored?.clientRevision ?: 0) + 1
                dao.saveProfile(
                    ProfileEntity(
                        heightCm = profile.heightCm,
                        sex = profile.sex,
                        hrMaxBpm = profile.maxHeartRateBpm,
                        birthDate = profile.birthDate,
                        clientRevision = revision,
                    ),
                )
                outbox.enqueueProfileEdit(
                    QueuedProfile(profile.heightCm, profile.sex, profile.maxHeartRateBpm, profile.birthDate, revision),
                )
                true
            }
        if (queued) scheduler.schedule()
        return ProfileSave.Saved
    }

    suspend fun recordBodyWeight(date: LocalDate, weightKg: String): BodyWeightSave {
        val weight =
            weightKg.toOptionalDecimal()?.takeIf { it.isFinite() && it > 0 } ?: return BodyWeightSave.InvalidWeight
        val day = date.toString()
        val saved =
            database.withTransaction {
                dao.bodyWeightOn(day)?.let { return@withTransaction BodyWeightSave.AlreadyRecorded(it.toDomain()) }
                dao.insertBodyWeight(listOf(BodyWeightEntity(day, weight)))
                outbox.enqueueBodyWeight(QueuedBodyWeight(day, weight))
                BodyWeightSave.Saved
            }
        if (saved == BodyWeightSave.Saved) scheduler.schedule()
        return saved
    }

    suspend fun useBodyWeightOnServer(date: String) {
        val requeued =
            database.withTransaction {
                val row = dao.bodyWeightOn(date) ?: return@withTransaction false
                outbox.useBodyWeightOnServer(date, row.weightKg)
            }
        if (requeued) scheduler.schedule()
    }
}
