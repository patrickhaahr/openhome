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

/** An alarm this far behind its deadline lost the moment, as after a force-stop: it stays silent. */
private const val LATE_ALARM_LIMIT_MILLIS = 60_000L

/** How an Exercise is rested, remembered until changed. */
data class RestPreference(val seconds: Long = DEFAULT_REST_SECONDS, val enabled: Boolean = true)

data class RestTimer(val exerciseId: Long, val exerciseName: String, val deadlineAt: Long)

/**
 * The rest between Sets. Storage holds a deadline, so nothing here depends on a screen or a process
 * being alive: the remaining time is read from the clock and Android's alarm ends the rest.
 *
 * Three facts stay separate: the stored timer (a deadline and whether it was alerted), the Exercise's
 * [RestPreference], and Android's one alarm, which only mirrors the timer through [syncAlarm]. The timer
 * row is the owner: a rest is running (not alerted), over (alerted, or its row gone) and is never
 * revived, and every change to it runs in one transaction. Replacing, adjusting, disabling, ending or
 * finishing the Workout each end in [syncAlarm], so a stale alarm is at worst a no-op in [onAlarm]. A
 * process that dies before [syncAlarm] is repaired by the sync at app start. Nothing promises a vibration
 * once Android has dropped the alarm: the rest is then shown over, silently.
 */
class RestTimers(private val database: OpenHomeDatabase, private val alarm: RestAlarm) {
    private val dao = database.restDao()

    fun observe(): Flow<RestTimer?> =
        dao.observeTimer()
            .map { row -> row?.let { RestTimer(it.timer.exerciseId, it.exerciseName, it.timer.deadlineAt) } }
            .distinctUntilChanged()

    /**
     * For an explicit confirmation, within its transaction; call [syncAlarm] after the commit. A rest
     * already being timed gives way to this one, and an Exercise with rest turned off leaves it alone.
     */
    suspend fun startFor(exerciseId: Long) {
        val preference = dao.loadPreference(exerciseId).toDomain()
        if (!preference.enabled) return
        val deadlineAt = System.currentTimeMillis() + preference.seconds * MILLIS_PER_SECOND
        dao.saveTimer(RestTimerEntity(exerciseId = exerciseId, deadlineAt = deadlineAt))
    }

    /** Remembers the new duration, and moves the deadline of this Exercise's rest if one is running. */
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

    /** Turning an Exercise's rest off also ends its rest in progress. */
    suspend fun setEnabled(exerciseId: Long, enabled: Boolean) {
        database.withTransaction {
            val preference = dao.loadPreference(exerciseId).toDomain()
            dao.savePreference(ExerciseRestEntity(exerciseId, preference.seconds, enabled))
            if (!enabled && dao.loadTimer()?.exerciseId == exerciseId) dao.deleteTimer()
        }
        syncAlarm()
    }

    /** Skips a running rest, or clears one that is over. */
    suspend fun end() {
        dao.deleteTimer()
        syncAlarm()
    }

    /** Idempotent: one alarm at the current deadline, or none. Android drops alarms on a force-stop or restart. */
    suspend fun syncAlarm() {
        val timer = dao.loadTimer()
        if (timer == null || timer.alerted) alarm.cancel() else alarm.setAt(timer.deadlineAt)
    }

    /**
     * Android says the deadline came. Returns whether to vibrate now, at most once per rest: the row
     * records it first, so anything going wrong afterwards loses a vibration rather than repeats one.
     */
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
        // The clock can have moved back since the alarm was set.
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
