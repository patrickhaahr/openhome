package com.patrickhaahr.openhome.storage

import androidx.room.ColumnInfo
import androidx.room.Embedded
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey
import androidx.room.Relation

/*
 * Primary keys are local and references use them. `server_id` is the Axum id when the record has one,
 * so records created on the device can later exist before the server has assigned an id.
 */

@Entity(tableName = "exercises", indices = [Index(value = ["server_id"], unique = true)])
data class ExerciseEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "server_id") val serverId: Long?,
    val name: String,
    val category: String,
    @ColumnInfo(name = "muscle_group") val muscleGroup: String?,
    val equipment: String?,
)

@Entity(
    tableName = "workouts",
    indices = [Index(value = ["server_id"], unique = true), Index(value = ["date", "id"])],
)
data class WorkoutEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "server_id") val serverId: Long?,
    val date: String,
    val name: String?,
    val notes: String?,
)

@Entity(
    tableName = "workout_entries",
    foreignKeys =
        [
            ForeignKey(
                entity = WorkoutEntity::class,
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
data class WorkoutEntryEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "workout_id") val workoutId: Long,
    @ColumnInfo(name = "exercise_id") val exerciseId: Long,
    @ColumnInfo(name = "order_index") val orderIndex: Long,
    val notes: String?,
)

@Entity(
    tableName = "workout_sets",
    foreignKeys =
        [
            ForeignKey(
                entity = WorkoutEntryEntity::class,
                parentColumns = ["id"],
                childColumns = ["entry_id"],
                onDelete = ForeignKey.CASCADE,
            ),
        ],
    indices = [Index("entry_id")],
)
data class WorkoutSetEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "entry_id") val entryId: Long,
    @ColumnInfo(name = "set_number") val setNumber: Long,
    val reps: Long?,
    @ColumnInfo(name = "weight_kg") val weightKg: Double?,
    @ColumnInfo(name = "duration_seconds") val durationSeconds: Long?,
    val rpe: Long?,
    val notes: String?,
)

/** One Body Weight per date, as on the server. */
@Entity(tableName = "body_weight")
data class BodyWeightEntity(
    @PrimaryKey val date: String,
    @ColumnInfo(name = "weight_kg") val weightKg: Double,
)

/** The single fitness Profile row. */
@Entity(tableName = "profile")
data class ProfileEntity(
    @PrimaryKey val id: Int = SINGLE_ROW_ID,
    @ColumnInfo(name = "height_cm") val heightCm: Double?,
    val sex: String?,
    @ColumnInfo(name = "hr_max_bpm") val hrMaxBpm: Long?,
    @ColumnInfo(name = "birth_date") val birthDate: String?,
)

const val SINGLE_ROW_ID = 1

data class WorkoutSummaryRow(
    val id: Long,
    val date: String,
    val name: String?,
    @ColumnInfo(name = "exercise_names") val exerciseNames: String?,
    @ColumnInfo(name = "set_count") val setCount: Int,
)

data class WorkoutWithEntries(
    @Embedded val workout: WorkoutEntity,
    @Relation(entity = WorkoutEntryEntity::class, parentColumn = "id", entityColumn = "workout_id")
    val entries: List<EntryWithDetails>,
)

data class EntryWithDetails(
    @Embedded val entry: WorkoutEntryEntity,
    @Relation(parentColumn = "exercise_id", entityColumn = "id")
    val exercise: ExerciseEntity,
    @Relation(parentColumn = "id", entityColumn = "entry_id")
    val sets: List<WorkoutSetEntity>,
)

/** One Exercise's sets within a logged Workout, with the day they were performed. */
data class LoggedEntryWithSets(
    @Embedded val entry: WorkoutEntryEntity,
    @ColumnInfo(name = "workout_date") val workoutDate: String,
    @Relation(parentColumn = "id", entityColumn = "entry_id")
    val sets: List<WorkoutSetEntity>,
)
