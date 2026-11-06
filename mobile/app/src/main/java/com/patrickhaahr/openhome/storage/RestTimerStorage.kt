package com.patrickhaahr.openhome.storage

import androidx.room.ColumnInfo
import androidx.room.Dao
import androidx.room.Embedded
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.PrimaryKey
import androidx.room.Query
import kotlinx.coroutines.flow.Flow

/** How one Exercise is rested; an Exercise without a row has the defaults. Stays on this device. */
@Entity(
    tableName = "exercise_rest",
    foreignKeys =
        [
            ForeignKey(
                entity = ExerciseEntity::class,
                parentColumns = ["id"],
                childColumns = ["exercise_id"],
                onDelete = ForeignKey.CASCADE,
            ),
        ],
)
data class ExerciseRestEntity(
    @PrimaryKey @ColumnInfo(name = "exercise_id") val exerciseId: Long,
    @ColumnInfo(name = "duration_seconds") val durationSeconds: Long,
    val enabled: Boolean,
)

/**
 * The one rest being timed. Its key is the Active Workout's, so there can never be two and ending the
 * Workout ends it. Only the deadline is stored: what remains is always worked out from the clock.
 */
@Entity(
    tableName = "rest_timer",
    foreignKeys =
        [
            ForeignKey(
                entity = ActiveWorkoutEntity::class,
                parentColumns = ["id"],
                childColumns = ["id"],
                onDelete = ForeignKey.CASCADE,
            ),
            ForeignKey(
                entity = ExerciseEntity::class,
                parentColumns = ["id"],
                childColumns = ["exercise_id"],
                onDelete = ForeignKey.CASCADE,
            ),
        ],
    indices = [Index("exercise_id")],
)
data class RestTimerEntity(
    @PrimaryKey val id: Int = SINGLE_ROW_ID,
    @ColumnInfo(name = "exercise_id") val exerciseId: Long,
    /** Wall-clock milliseconds, the one clock that keeps counting across a restart of the phone. */
    @ColumnInfo(name = "deadline_at") val deadlineAt: Long,
    /** Whether this rest's end has been dealt with, so it can never vibrate twice. */
    val alerted: Boolean = false,
)

data class RestTimerWithExercise(
    @Embedded val timer: RestTimerEntity,
    @ColumnInfo(name = "exercise_name") val exerciseName: String,
)

@Dao
interface RestDao {
    @Query(
        """
        SELECT rest_timer.*, exercises.name AS exercise_name FROM rest_timer
        JOIN exercises ON exercises.id = rest_timer.exercise_id
        """,
    )
    fun observeTimer(): Flow<RestTimerWithExercise?>

    @Query("SELECT * FROM rest_timer")
    suspend fun loadTimer(): RestTimerEntity?

    /** Replaces any earlier rest: there is only ever one being timed. */
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun saveTimer(timer: RestTimerEntity)

    @Query("DELETE FROM rest_timer")
    suspend fun deleteTimer()

    @Query("SELECT * FROM exercise_rest WHERE exercise_id = :exerciseId")
    suspend fun loadPreference(exerciseId: Long): ExerciseRestEntity?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun savePreference(preference: ExerciseRestEntity)

    /** What was set on the row being merged away wins: it is the one the user has been logging with. */
    @Query("UPDATE OR REPLACE exercise_rest SET exercise_id = :to WHERE exercise_id = :from")
    suspend fun movePreference(from: Long, to: Long)

    @Query("UPDATE rest_timer SET exercise_id = :to WHERE exercise_id = :from")
    suspend fun moveTimer(from: Long, to: Long)
}
