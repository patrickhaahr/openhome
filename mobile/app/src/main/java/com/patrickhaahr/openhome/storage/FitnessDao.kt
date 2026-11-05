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

    @Transaction
    @Query("SELECT * FROM workouts WHERE id = :id")
    suspend fun loadWorkout(id: Long): WorkoutWithEntries?

    /** The Exercise's most recent logged entry that has Sets, whichever Workout it is in. */
    @Transaction
    @Query(
        """
        SELECT we.*, w.date AS workout_date FROM workout_entries we
        JOIN workouts w ON w.id = we.workout_id
        WHERE we.exercise_id = :exerciseId
            AND EXISTS (SELECT 1 FROM workout_sets s WHERE s.entry_id = we.id)
        ORDER BY w.date DESC, w.id DESC, we.order_index DESC, we.id DESC
        LIMIT 1
        """,
    )
    fun observeLatestLoggedEntry(exerciseId: Long): Flow<LoggedEntryWithSets?>

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

    @Query("SELECT * FROM exercises WHERE id = :exerciseId")
    suspend fun loadExercise(exerciseId: Long): ExerciseEntity?

    @Query("SELECT id FROM exercises WHERE server_id = :serverId")
    suspend fun exerciseHoldingServerId(serverId: Long): Long?

    @Query("SELECT name FROM exercises WHERE id != :exceptId")
    suspend fun exerciseNamesExcept(exceptId: Long): List<String>

    @Query(
        """
        UPDATE exercises SET name = :name, category = :category, muscle_group = :muscleGroup, equipment = :equipment
        WHERE id = :exerciseId
        """,
    )
    suspend fun updateExerciseDetails(
        exerciseId: Long,
        name: String,
        category: String,
        muscleGroup: String?,
        equipment: String?,
    )

    @Query(
        """
        UPDATE exercises
        SET server_id = :serverId, name = :name, category = :category, muscle_group = :muscleGroup, equipment = :equipment
        WHERE id = :exerciseId
        """,
    )
    suspend fun applyServerExercise(
        exerciseId: Long,
        serverId: Long,
        name: String,
        category: String,
        muscleGroup: String?,
        equipment: String?,
    )

    @Query("UPDATE workout_entries SET exercise_id = :to WHERE exercise_id = :from")
    suspend fun moveWorkoutEntries(from: Long, to: Long)

    @Query("UPDATE active_entries SET exercise_id = :to WHERE exercise_id = :from")
    suspend fun moveActiveEntries(from: Long, to: Long)

    @Query("DELETE FROM exercises WHERE id = :exerciseId")
    suspend fun deleteExercise(exerciseId: Long)

    @Query("UPDATE workouts SET server_id = :serverId WHERE id = :workoutId")
    suspend fun setWorkoutServerId(workoutId: Long, serverId: Long)

    @Insert
    suspend fun insertExercises(exercises: List<ExerciseEntity>): List<Long>

    @Insert
    suspend fun insertExercise(exercise: ExerciseEntity): Long

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
