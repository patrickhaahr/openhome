package com.patrickhaahr.openhome.fitness

import androidx.room.withTransaction
import com.patrickhaahr.openhome.storage.ExerciseRestEntity
import com.patrickhaahr.openhome.storage.OpenHomeDatabase
import com.patrickhaahr.openhome.storage.RestTimerEntity
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map

const val DEFAULT_REST_SECONDS = 150L
const val REST_STEP_SECONDS = 15L
private const val MIN_REST_SECONDS = 15L
private const val MAX_REST_SECONDS = 900L
private const val MILLIS_PER_SECOND = 1_000L

private const val LATE_ALARM_LIMIT_MILLIS = 60_000L

data class RestPreference(val seconds: Long = DEFAULT_REST_SECONDS, val enabled: Boolean = true)

data class RestTimer(val exerciseId: Long, val exerciseName: String, val deadlineAt: Long)

class RestTimers(private val database: OpenHomeDatabase, private val alarm: RestAlarm) {
    private val dao = database.restDao()

    fun observe(): Flow<RestTimer?> =
        dao.observeTimer()
            .map { row -> row?.let { RestTimer(it.timer.exerciseId, it.exerciseName, it.timer.deadlineAt) } }
            .distinctUntilChanged()

    suspend fun startFor(exerciseId: Long) {
        val preference = dao.loadPreference(exerciseId).toDomain()
        if (!preference.enabled) return
        val deadlineAt = System.currentTimeMillis() + preference.seconds * MILLIS_PER_SECOND
        dao.saveTimer(RestTimerEntity(exerciseId = exerciseId, deadlineAt = deadlineAt))
    }

    suspend fun adjust(exerciseId: Long, deltaSeconds: Long) {
        database.withTransaction {
            val preference = dao.loadPreference(exerciseId).toDomain()
            val seconds = (preference.seconds + deltaSeconds).coerceIn(MIN_REST_SECONDS, MAX_REST_SECONDS)
            dao.savePreference(ExerciseRestEntity(exerciseId, seconds, preference.enabled))
            val timer = dao.loadTimer()
            if (timer != null && timer.exerciseId == exerciseId && timer.deadlineAt > System.currentTimeMillis()) {
                dao.saveTimer(timer.copy(deadlineAt = timer.deadlineAt + (seconds - preference.seconds) * MILLIS_PER_SECOND))
            }
        }
        syncAlarm()
    }

    suspend fun setEnabled(exerciseId: Long, enabled: Boolean) {
        database.withTransaction {
            val preference = dao.loadPreference(exerciseId).toDomain()
            dao.savePreference(ExerciseRestEntity(exerciseId, preference.seconds, enabled))
            if (!enabled && dao.loadTimer()?.exerciseId == exerciseId) dao.deleteTimer()
        }
        syncAlarm()
    }

    suspend fun end() {
        dao.deleteTimer()
        syncAlarm()
    }

    suspend fun syncAlarm() {
        val timer = dao.loadTimer()
        if (timer == null || timer.alerted) alarm.cancel() else alarm.setAt(timer.deadlineAt)
    }

    suspend fun onAlarm(): Boolean {
        val now = System.currentTimeMillis()
        val outcome =
            database.withTransaction {
                val timer = dao.loadTimer()
                when {
                    timer == null || timer.alerted -> AlarmOutcome.NOTHING
                    timer.deadlineAt > now -> AlarmOutcome.NOT_YET
                    else -> {
                        dao.saveTimer(timer.copy(alerted = true))
                        if (now - timer.deadlineAt <= LATE_ALARM_LIMIT_MILLIS) AlarmOutcome.VIBRATE else AlarmOutcome.NOTHING
                    }
                }
            }
        if (outcome == AlarmOutcome.NOT_YET) syncAlarm()
        return outcome == AlarmOutcome.VIBRATE
    }

    private enum class AlarmOutcome {
        NOTHING,
        NOT_YET,
        VIBRATE,
    }
}

internal fun ExerciseRestEntity?.toDomain(): RestPreference =
    this?.let { RestPreference(it.durationSeconds, it.enabled) } ?: RestPreference()
