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
import androidx.room.Relation
import androidx.room.Transaction
import androidx.room.Update
import kotlinx.coroutines.flow.Flow

/** Whether a Set is logged as reps or as a Timed Hold; a choice per row, not an Exercise type. */
enum class SetMeasure {
    REPS,
    HOLD,
}

/** The single row is what limits the app to one Active Workout. It is never synchronized. */
@Entity(tableName = "active_workout")
data class ActiveWorkoutEntity(
    @PrimaryKey val id: Int = SINGLE_ROW_ID,
    val date: String,
    val name: String,
    val notes: String,
)

@Entity(
    tableName = "active_entries",
    foreignKeys =
        [
            ForeignKey(
                entity = ActiveWorkoutEntity::class,
                parentColumns = ["id"],
                childColumns = ["workout_id"],
                onDelete = ForeignKey.CASCADE,
            ),
            ForeignKey(
                entity = ExerciseEntity::class,
                parentColumns = ["id"],
                childColumns = ["exercise_id"],
            ),
        ],
    indices = [Index("workout_id"), Index("exercise_id")],
)
data class ActiveEntryEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "workout_id") val workoutId: Int = SINGLE_ROW_ID,
    @ColumnInfo(name = "exercise_id") val exerciseId: Long,
    @ColumnInfo(name = "order_index") val orderIndex: Long,
)

/**
 * Values are kept as typed, so a half-entered number survives process death unchanged. They become
 * numbers only when a Set is confirmed; `completed` is never true for a row that does not parse.
 */
@Entity(
    tableName = "active_sets",
    foreignKeys =
        [
            ForeignKey(
                entity = ActiveEntryEntity::class,
                parentColumns = ["id"],
                childColumns = ["entry_id"],
                onDelete = ForeignKey.CASCADE,
            ),
        ],
    indices = [Index("entry_id")],
)
data class ActiveSetEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "entry_id") val entryId: Long,
    @ColumnInfo(name = "set_number") val setNumber: Long,
    val measure: SetMeasure,
    val reps: String = "",
    @ColumnInfo(name = "weight_kg") val weightKg: String = "",
    @ColumnInfo(name = "duration_seconds") val durationSeconds: String = "",
    val rpe: String = "",
    val completed: Boolean = false,
)

data class ActiveWorkoutWithEntries(
    @Embedded val workout: ActiveWorkoutEntity,
    @Relation(entity = ActiveEntryEntity::class, parentColumn = "id", entityColumn = "workout_id")
    val entries: List<ActiveEntryWithDetails>,
)

data class ActiveEntryWithDetails(
    @Embedded val entry: ActiveEntryEntity,
    @Relation(parentColumn = "exercise_id", entityColumn = "id")
    val exercise: ExerciseEntity,
    @Relation(parentColumn = "id", entityColumn = "entry_id")
    val sets: List<ActiveSetEntity>,
)

@Dao
interface ActiveWorkoutDao {
    @Transaction
    @Query("SELECT * FROM active_workout WHERE id = $SINGLE_ROW_ID")
    fun observe(): Flow<ActiveWorkoutWithEntries?>

    @Transaction
    @Query("SELECT * FROM active_workout WHERE id = $SINGLE_ROW_ID")
    suspend fun load(): ActiveWorkoutWithEntries?

    /** Ignored when an Active Workout exists, so starting can never replace one. */
    @Insert(onConflict = OnConflictStrategy.IGNORE)
    suspend fun insertWorkout(workout: ActiveWorkoutEntity)

    @Query("UPDATE active_workout SET date = :date")
    suspend fun setDate(date: String)

    @Query("UPDATE active_workout SET name = :name")
    suspend fun setName(name: String)

    @Query("UPDATE active_workout SET notes = :notes")
    suspend fun setNotes(notes: String)

    @Query("DELETE FROM active_workout")
    suspend fun deleteWorkout()

    @Query("SELECT COALESCE(MAX(order_index) + 1, 0) FROM active_entries")
    suspend fun nextOrderIndex(): Long

    @Insert
    suspend fun insertEntry(entry: ActiveEntryEntity): Long

    @Query("DELETE FROM active_entries WHERE id = :entryId")
    suspend fun deleteEntry(entryId: Long)

    @Insert
    suspend fun insertSet(set: ActiveSetEntity): Long

    @Update
    suspend fun updateSet(set: ActiveSetEntity)

    @Query("SELECT * FROM active_sets WHERE id = :setId")
    suspend fun loadSet(setId: Long): ActiveSetEntity?

    @Query("SELECT * FROM active_sets WHERE entry_id = :entryId ORDER BY set_number")
    suspend fun loadSets(entryId: Long): List<ActiveSetEntity>
}
