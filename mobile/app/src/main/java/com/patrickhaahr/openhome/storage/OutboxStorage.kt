package com.patrickhaahr.openhome.storage

import androidx.room.ColumnInfo
import androidx.room.Dao
import androidx.room.Entity
import androidx.room.Index
import androidx.room.Insert
import androidx.room.PrimaryKey
import androidx.room.Query
import java.time.LocalDate
import kotlinx.coroutines.flow.Flow

enum class OutboxKind {
    CREATE_WORKOUT,
    CREATE_EXERCISE,
    EDIT_WORKOUT,

    CREATE_BODY_WEIGHT,
    EDIT_PROFILE,
}

fun bodyWeightRecordId(date: LocalDate): Long = date.toEpochDay()

enum class OutboxState {
    PENDING,

    REFUSED,
}

enum class SyncProblem {
    UNREACHABLE,
    UNAUTHORIZED,
    SERVER_ERROR,
    API_KEY_UNAVAILABLE,
    EXERCISE_NOT_SYNCED,

    WORKOUT_NOT_SYNCED,
    REFUSED,

    NAME_TAKEN,

    DATE_TAKEN,
}

@Entity(
    tableName = "fitness_outbox",
    indices = [Index(value = ["request_id"], unique = true), Index(value = ["kind", "record_id"])],
)
data class OutboxEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val kind: OutboxKind,
    @ColumnInfo(name = "record_id") val recordId: Long,
    @ColumnInfo(name = "request_id") val requestId: String,
    val payload: String,
    val state: OutboxState = OutboxState.PENDING,
    val problem: SyncProblem? = null,
    val attempts: Int = 0,
)

data class OutboxStatusRow(
    val id: Long,
    val kind: OutboxKind,
    @ColumnInfo(name = "record_id") val recordId: Long,
    val state: OutboxState,
    val problem: SyncProblem?,
)

@Dao
interface OutboxDao {
    @Insert
    suspend fun insert(entry: OutboxEntity): Long

    @Query("SELECT id, kind, record_id, state, problem FROM fitness_outbox ORDER BY id")
    fun observeStatus(): Flow<List<OutboxStatusRow>>

    @Query("SELECT * FROM fitness_outbox WHERE state = 'PENDING' AND id > :afterId ORDER BY id LIMIT 1")
    suspend fun nextPending(afterId: Long): OutboxEntity?

    @Query("SELECT COUNT(*) FROM fitness_outbox WHERE state = 'PENDING'")
    suspend fun pendingCount(): Int

    @Query("UPDATE fitness_outbox SET problem = :problem, attempts = attempts + 1 WHERE id = :id")
    suspend fun recordProblem(id: Long, problem: SyncProblem)

    @Query("UPDATE fitness_outbox SET problem = :problem WHERE state = 'PENDING'")
    suspend fun recordProblemOnPending(problem: SyncProblem)

    @Query("UPDATE fitness_outbox SET state = 'REFUSED', problem = :problem, attempts = attempts + 1 WHERE id = :id")
    suspend fun markRefused(id: Long, problem: SyncProblem)

    @Query("SELECT problem FROM fitness_outbox WHERE kind = :kind AND record_id = :recordId AND state = 'REFUSED'")
    suspend fun refusalOf(kind: OutboxKind, recordId: Long): SyncProblem?

    @Query(
        """
        UPDATE fitness_outbox SET request_id = :requestId, payload = :payload, state = 'PENDING', problem = NULL, attempts = 0
        WHERE kind = :kind AND record_id = :recordId AND state = 'REFUSED'
        """,
    )
    suspend fun replaceRefused(kind: OutboxKind, recordId: Long, requestId: String, payload: String): Int

    @Query("DELETE FROM fitness_outbox WHERE kind = :kind AND record_id = :recordId AND state = 'REFUSED'")
    suspend fun deleteRefused(kind: OutboxKind, recordId: Long): Int

    @Query("SELECT * FROM fitness_outbox WHERE kind IN (:kinds)")
    suspend fun allOfKinds(kinds: List<OutboxKind>): List<OutboxEntity>

    @Query("DELETE FROM fitness_outbox WHERE kind = :kind AND record_id = :recordId")
    suspend fun deleteAllOf(kind: OutboxKind, recordId: Long)

    @Query("UPDATE fitness_outbox SET payload = :payload WHERE id = :id")
    suspend fun updatePayload(id: Long, payload: String)

    @Query("UPDATE fitness_outbox SET state = 'PENDING', problem = NULL WHERE state = 'REFUSED'")
    suspend fun requeueRefused()

    @Query("DELETE FROM fitness_outbox WHERE id = :id")
    suspend fun delete(id: Long)
}
