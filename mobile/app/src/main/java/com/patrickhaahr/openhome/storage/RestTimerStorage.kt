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
    @ColumnInfo(name = "deadline_at") val deadlineAt: Long,
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

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun saveTimer(timer: RestTimerEntity)

    @Query("DELETE FROM rest_timer")
    suspend fun deleteTimer()

    @Query("SELECT * FROM exercise_rest WHERE exercise_id = :exerciseId")
    suspend fun loadPreference(exerciseId: Long): ExerciseRestEntity?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun savePreference(preference: ExerciseRestEntity)

    @Query("UPDATE OR REPLACE exercise_rest SET exercise_id = :to WHERE exercise_id = :from")
    suspend fun movePreference(from: Long, to: Long)

    @Query("UPDATE rest_timer SET exercise_id = :to WHERE exercise_id = :from")
    suspend fun moveTimer(from: Long, to: Long)
}
