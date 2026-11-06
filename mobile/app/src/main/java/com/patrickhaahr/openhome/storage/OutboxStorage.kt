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

/** A fitness change Axum has not acknowledged. Add a value here for each new kind of change. */
enum class OutboxKind {
    CREATE_WORKOUT,
    CREATE_EXERCISE,
    EDIT_WORKOUT,

    /** Its record id is the Body Weight's date as an epoch day: see [bodyWeightRecordId]. */
    CREATE_BODY_WEIGHT,
    EDIT_PROFILE,
}

/** A Body Weight is identified by its date, and an outbox row refers to its record by number. */
fun bodyWeightRecordId(date: LocalDate): Long = date.toEpochDay()

enum class OutboxState {
    /** Delivered in id order once Wi-Fi and Axum allow it. */
    PENDING,

    /** Axum refused it; it waits, with its local record intact, until the user retries. */
    REFUSED,
}

/** Why the last delivery attempt did not end in an acknowledgement. */
enum class SyncProblem {
    UNREACHABLE,
    UNAUTHORIZED,
    SERVER_ERROR,
    API_KEY_UNAVAILABLE,
    EXERCISE_NOT_SYNCED,

    /** A correction waiting behind the create that gives its Workout a server id. */
    WORKOUT_NOT_SYNCED,
    REFUSED,

    /** Axum refused a new Exercise because it already holds one under that exact name. */
    NAME_TAKEN,

    /** Axum refused a new Body Weight because it already holds one for that date. */
    DATE_TAKEN,
}

/**
 * One queued change, deleted in the transaction that records its acknowledgement. [payload] is frozen
 * when queued, so a retry repeats the same arguments under the same [requestId] whatever has happened
 * to the local record since. Later corrections are later rows, never edits to this one; the exception
 * is a refused create, which Axum never accepted: correcting it replaces both here, keeping its place
 * ahead of whatever depends on it.
 */
@Entity(
    tableName = "fitness_outbox",
    indices = [Index(value = ["request_id"], unique = true), Index(value = ["kind", "record_id"])],
)
data class OutboxEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val kind: OutboxKind,
    /** The local id of the record this change belongs to. */
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

    /** Returns how many rows changed: none when the change is no longer refused. */
    @Query(
        """
        UPDATE fitness_outbox SET request_id = :requestId, payload = :payload, state = 'PENDING', problem = NULL, attempts = 0
        WHERE kind = :kind AND record_id = :recordId AND state = 'REFUSED'
        """,
    )
    suspend fun replaceRefused(kind: OutboxKind, recordId: Long, requestId: String, payload: String): Int

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
