package com.patrickhaahr.openhome.setup

import androidx.room.withTransaction
import com.patrickhaahr.openhome.api.ApiBodyWeight
import com.patrickhaahr.openhome.api.ApiExercise
import com.patrickhaahr.openhome.api.ApiFailure
import com.patrickhaahr.openhome.api.ApiProfile
import com.patrickhaahr.openhome.api.ApiWorkoutDetail
import com.patrickhaahr.openhome.api.OpenHomeApi
import com.patrickhaahr.openhome.storage.BodyWeightEntity
import com.patrickhaahr.openhome.storage.ExerciseEntity
import com.patrickhaahr.openhome.storage.OpenHomeDatabase
import com.patrickhaahr.openhome.storage.ProfileEntity
import com.patrickhaahr.openhome.storage.SetupStateEntity
import com.patrickhaahr.openhome.storage.SetupStatus
import com.patrickhaahr.openhome.storage.WorkoutEntity
import com.patrickhaahr.openhome.storage.WorkoutEntryEntity
import com.patrickhaahr.openhome.storage.WorkoutSetEntity
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.OkHttpClient

private const val CONCURRENT_DETAIL_REQUESTS = 4

sealed interface SetupState {
    data object NotStarted : SetupState

    /** A previous attempt stopped mid-download, possibly with the process; nothing from it was saved. */
    data class Interrupted(val baseUrl: String) : SetupState

    data object Complete : SetupState
}

sealed interface SetupProgress {
    data object CheckingConnection : SetupProgress

    data object ListingWorkouts : SetupProgress

    data class DownloadingWorkouts(val done: Int, val total: Int) : SetupProgress

    data object DownloadingLibraryAndProfile : SetupProgress

    data object Saving : SetupProgress
}

data class SavedConnection(val baseUrl: String, val apiKeyAvailable: Boolean)

/**
 * The online Setup Flow. Everything downloaded commits in one transaction together with the COMPLETE
 * status, so an interruption at any point leaves no partial fitness data and no claim of completion.
 *
 * [run] is one linear sequence (check, mark DOWNLOADING, download, seal the key, commit), already clear as
 * straight-line code, so it has no state machine of its own. Progress is reported to the caller, which owns
 * the on-screen outcome. The persisted [SetupStatus] is a separate fact that survives the process: a
 * run that fails the connection check leaves the status as it was; one cancelled or failed after DOWNLOADING is written
 * leaves DOWNLOADING (no key, no data). Either way a retry simply starts the sequence again.
 */
class SetupService(
    private val database: OpenHomeDatabase,
    private val cipher: ApiKeyCipher,
    private val httpClient: OkHttpClient,
) {
    private val setupDao = database.setupDao()
    private val fitnessDao = database.fitnessDao()

    fun observeState(): Flow<SetupState> =
        setupDao.observe().map { row ->
            when (row?.status) {
                null -> SetupState.NotStarted
                SetupStatus.DOWNLOADING -> SetupState.Interrupted(row.baseUrl)
                SetupStatus.COMPLETE -> SetupState.Complete
            }
        }

    suspend fun savedConnection(): SavedConnection? {
        val row = completedSetup() ?: return null
        return SavedConnection(row.baseUrl, apiKeyAvailable = openApiKey(row) != null)
    }

    /** What background work connects with; null when setup is unfinished or the key can't be opened. */
    suspend fun connection(): Connection? {
        val row = completedSetup() ?: return null
        val apiKey = openApiKey(row) ?: return null
        val baseUrl = row.baseUrl.toHttpUrlOrNull() ?: return null
        return Connection(baseUrl, apiKey)
    }

    private suspend fun completedSetup(): SetupStateEntity? =
        setupDao.observe().first()?.takeIf { it.status == SetupStatus.COMPLETE }

    private fun openApiKey(row: SetupStateEntity): String? {
        val ciphertext = row.apiKeyCiphertext
        val iv = row.apiKeyIv
        return if (ciphertext != null && iv != null) cipher.open(SealedApiKey(ciphertext, iv)) else null
    }

    /** @throws ApiFailure when the Axum API cannot be used; storage and Keystore errors propagate too. */
    suspend fun run(connection: Connection, onProgress: (SetupProgress) -> Unit) {
        val api = OpenHomeApi(connection.baseUrl, connection.apiKey, httpClient)

        onProgress(SetupProgress.CheckingConnection)
        api.checkConnection()

        val baseUrl = connection.baseUrl.toString().trimEnd('/')
        setupDao.save(
            SetupStateEntity(
                status = SetupStatus.DOWNLOADING,
                baseUrl = baseUrl,
                apiKeyCiphertext = null,
                apiKeyIv = null,
            ),
        )

        // Exercises come after Workouts so that every Exercise a downloaded Workout references is listed.
        onProgress(SetupProgress.ListingWorkouts)
        val workouts = downloadWorkouts(api, onProgress)
        onProgress(SetupProgress.DownloadingLibraryAndProfile)
        val bodyWeight = api.allBodyWeight()
        val profile = api.profile()
        val exercises = api.exercises()

        onProgress(SetupProgress.Saving)
        val sealed = cipher.seal(connection.apiKey)
        database.withTransaction {
            storeDownload(exercises, workouts, bodyWeight, profile)
            setupDao.save(
                SetupStateEntity(
                    status = SetupStatus.COMPLETE,
                    baseUrl = baseUrl,
                    apiKeyCiphertext = sealed.ciphertext,
                    apiKeyIv = sealed.iv,
                ),
            )
        }
    }

    private suspend fun downloadWorkouts(
        api: OpenHomeApi,
        onProgress: (SetupProgress) -> Unit,
    ): List<ApiWorkoutDetail> = coroutineScope {
        val summaries = api.allWorkoutSummaries()
        val done = AtomicInteger()
        val permits = Semaphore(CONCURRENT_DETAIL_REQUESTS)
        onProgress(SetupProgress.DownloadingWorkouts(0, summaries.size))
        summaries
            .map { summary ->
                async {
                    permits.withPermit {
                        api.workoutDetail(summary.id).also {
                            onProgress(SetupProgress.DownloadingWorkouts(done.incrementAndGet(), summaries.size))
                        }
                    }
                }
            }
            .awaitAll()
    }

    private suspend fun storeDownload(
        exercises: List<ApiExercise>,
        workouts: List<ApiWorkoutDetail>,
        bodyWeight: List<ApiBodyWeight>,
        profile: ApiProfile,
    ) {
        val localIds =
            exercises.map { it.id }
                .zip(
                    fitnessDao.insertExercises(
                        exercises.map {
                            ExerciseEntity(
                                serverId = it.id,
                                name = it.name,
                                category = it.category,
                                muscleGroup = it.muscleGroup,
                                equipment = it.equipment,
                            )
                        },
                    ),
                )
                .toMap()

        // Oldest first, so that local ids grow with the server's order of same-day Workouts.
        for (workout in workouts.sortedWith(compareBy({ it.date }, { it.id }))) {
            val workoutId =
                fitnessDao.insertWorkout(
                    WorkoutEntity(
                        serverId = workout.id,
                        date = workout.date,
                        name = workout.name,
                        notes = workout.notes,
                        clientRevision = workout.clientRevision,
                    ),
                )
            for (entry in workout.exercises) {
                val entryId =
                    fitnessDao.insertEntry(
                        WorkoutEntryEntity(
                            workoutId = workoutId,
                            exerciseId =
                                localIds[entry.exercise.id] ?: throw ApiFailure.UnreadableResponse(),
                            orderIndex = entry.orderIndex,
                            notes = entry.notes,
                        ),
                    )
                fitnessDao.insertSets(
                    entry.sets.map {
                        WorkoutSetEntity(
                            entryId = entryId,
                            setNumber = it.setNumber,
                            reps = it.reps,
                            weightKg = it.weightKg,
                            durationSeconds = it.durationSeconds,
                            rpe = it.rpe,
                            notes = it.notes,
                        )
                    },
                )
            }
        }

        fitnessDao.insertBodyWeight(bodyWeight.map { BodyWeightEntity(it.date, it.weightKg) })
        fitnessDao.insertProfile(
            ProfileEntity(
                heightCm = profile.heightCm,
                sex = profile.sex,
                hrMaxBpm = profile.hrMaxBpm,
                birthDate = profile.birthDate,
                clientRevision = profile.clientRevision,
            ),
        )
    }
}
