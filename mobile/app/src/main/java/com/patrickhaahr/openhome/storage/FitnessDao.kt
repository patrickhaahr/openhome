package com.patrickhaahr.openhome.storage

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import androidx.room.Transaction
import kotlinx.coroutines.flow.Flow

private const val SUMMARY_COLUMNS =
    """
    w.id, w.date, w.name,
    (SELECT group_concat(name, ' · ') FROM (
        SELECT e.name AS name
        FROM workout_entries we JOIN exercises e ON e.id = we.exercise_id
        WHERE we.workout_id = w.id
        ORDER BY we.order_index, we.id
    )) AS exercise_names,
    (SELECT COUNT(*)
        FROM workout_sets s JOIN workout_entries we ON we.id = s.entry_id
        WHERE we.workout_id = w.id
    ) AS set_count
    """

@Dao
interface FitnessDao {
    @Query("SELECT $SUMMARY_COLUMNS FROM workouts w ORDER BY w.date DESC, w.id DESC")
    fun observeHistory(): Flow<List<WorkoutSummaryRow>>

    @Query("SELECT $SUMMARY_COLUMNS FROM workouts w ORDER BY w.date DESC, w.id DESC LIMIT 1")
    fun observeLatestWorkout(): Flow<WorkoutSummaryRow?>

    @Query("SELECT COUNT(*) FROM workouts")
    fun observeWorkoutCount(): Flow<Int>

    @Query("SELECT COUNT(*) FROM exercises")
    fun observeExerciseCount(): Flow<Int>

    @Transaction
    @Query("SELECT * FROM workouts WHERE id = :id")
    fun observeWorkout(id: Long): Flow<WorkoutWithEntries?>

    @Query("SELECT * FROM profile WHERE id = $SINGLE_ROW_ID")
    fun observeProfile(): Flow<ProfileEntity?>

    @Query("SELECT * FROM body_weight ORDER BY date DESC")
    fun observeBodyWeight(): Flow<List<BodyWeightEntity>>

    @Query("SELECT * FROM exercises ORDER BY name COLLATE NOCASE, id")
    fun observeExercises(): Flow<List<ExerciseEntity>>

    /** The most recently logged Set of an Exercise, whichever Workout it is in. */
    @Query(
        """
        SELECT s.* FROM workout_sets s
        JOIN workout_entries we ON we.id = s.entry_id
        JOIN workouts w ON w.id = we.workout_id
        WHERE we.exercise_id = :exerciseId
        ORDER BY w.date DESC, w.id DESC, we.order_index DESC, s.set_number DESC
        LIMIT 1
        """,
    )
    suspend fun latestSetOfExercise(exerciseId: Long): WorkoutSetEntity?

    @Query("SELECT server_id FROM exercises WHERE id = :exerciseId")
    suspend fun exerciseServerId(exerciseId: Long): Long?

    @Query("UPDATE workouts SET server_id = :serverId WHERE id = :workoutId")
    suspend fun setWorkoutServerId(workoutId: Long, serverId: Long)

    @Insert
    suspend fun insertExercises(exercises: List<ExerciseEntity>): List<Long>

    @Insert
    suspend fun insertWorkout(workout: WorkoutEntity): Long

    @Insert
    suspend fun insertEntry(entry: WorkoutEntryEntity): Long

    @Insert
    suspend fun insertSets(sets: List<WorkoutSetEntity>)

    @Insert
    suspend fun insertBodyWeight(entries: List<BodyWeightEntity>)

    @Insert
    suspend fun insertProfile(profile: ProfileEntity)
}
