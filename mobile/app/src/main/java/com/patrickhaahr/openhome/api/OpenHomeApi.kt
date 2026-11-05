package com.patrickhaahr.openhome.api

import java.io.IOException
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runInterruptible
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import okhttp3.HttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody

/** The largest page the Axum list endpoints serve; they clamp `limit` to it and default to 100. */
private const val PAGE_SIZE = 500

private val JSON_MEDIA_TYPE = "application/json".toMediaType()

/** Why a call to the Axum API did not produce a usable answer. */
sealed class ApiFailure(message: String, cause: Throwable? = null) : Exception(message, cause) {
    class Unreachable(cause: IOException) : ApiFailure("Axum API unreachable", cause)

    class Unauthorized : ApiFailure("Axum API rejected the API Key")

    class Rejected(val status: Int) : ApiFailure("Axum API answered HTTP $status")

    class UnreadableResponse(cause: Throwable? = null) : ApiFailure("Unreadable Axum API response", cause)

    /** More rows share one date than a page can hold, so `to`/`from` paging cannot pass them. */
    class HistoryCannotBePaged(val date: String) : ApiFailure("Cannot page past $date")
}

fun newOpenHomeHttpClient(): OkHttpClient =
    OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .build()

/** Authenticated calls to the Axum API; every failure surfaces as an [ApiFailure]. */
class OpenHomeApi(
    private val baseUrl: HttpUrl,
    private val apiKey: String,
    private val client: OkHttpClient,
) {
    private val json = Json { ignoreUnknownKeys = true }

    /** `/api/health` sits behind the API Key check, so success proves both the address and the key. */
    suspend fun checkConnection() {
        val health =
            try {
                get<HealthResponse>("api/health")
            } catch (failure: ApiFailure.Rejected) {
                if (failure.status == 404) throw ApiFailure.UnreadableResponse(failure) else throw failure
            }
        if (health.status != "ok") throw ApiFailure.UnreadableResponse()
    }

    suspend fun exercises(): List<ApiExercise> = get("api/exercises")

    suspend fun workoutDetail(id: Long): ApiWorkoutDetail = get("api/workouts/$id")

    suspend fun profile(): ApiProfile = get("api/profile")

    /** Answers with the same Workout however many times one `request_id` is delivered. */
    suspend fun createWorkout(workout: ApiWorkoutCreate): ApiWorkoutCreated =
        post("api/workouts", json.encodeToString(workout))

    /** Answers with the same Exercise however many times one `request_id` is delivered. */
    suspend fun createExercise(exercise: ApiExerciseCreate): ApiExercise =
        post("api/exercises", json.encodeToString(exercise))

    /** Delivering the same revision again, or an older one, changes nothing and answers the Workout as it is. */
    suspend fun editWorkout(id: Long, edit: ApiWorkoutEdit): ApiWorkoutCreated =
        send("PATCH", "api/workouts/$id", json.encodeToString(edit))

    private suspend inline fun <reified T> post(path: String, body: String): T = send("POST", path, body)

    private suspend inline fun <reified T> send(method: String, path: String, body: String): T {
        val request =
            Request.Builder()
                .url(baseUrl.newBuilder().addPathSegments(path).build())
                .header("Authorization", "Bearer $apiKey")
                .method(method, body.toRequestBody(JSON_MEDIA_TYPE))
                .build()
        return decode(execute(request))
    }

    /** The whole History, newest first: the list endpoint alone returns only its newest page. */
    suspend fun allWorkoutSummaries(): List<ApiWorkoutSummary> =
        collectAllPages(id = ApiWorkoutSummary::id, date = ApiWorkoutSummary::date) { boundary ->
            get("api/workouts", "limit" to PAGE_SIZE.toString(), "to" to boundary)
        }

    /** Every Body Weight record, oldest first: the list endpoint alone returns only its oldest page. */
    suspend fun allBodyWeight(): List<ApiBodyWeight> =
        collectAllPages(id = ApiBodyWeight::id, date = ApiBodyWeight::date) { boundary ->
            get("api/body_weight", "limit" to PAGE_SIZE.toString(), "from" to boundary)
        }

    /**
     * Walks a date-ordered list whose boundary is inclusive, because several rows can share a date.
     * It stops only on a page that adds nothing, so a smaller server page cannot end History early.
     */
    private suspend fun <T> collectAllPages(
        id: (T) -> Long,
        date: (T) -> String,
        page: suspend (boundary: String?) -> List<T>,
    ): List<T> {
        val collected = LinkedHashMap<Long, T>()
        var boundary: String? = null
        while (true) {
            val rows = page(boundary)
            val sizeBefore = collected.size
            for (row in rows) {
                if (id(row) !in collected) collected[id(row)] = row
            }
            if (collected.size == sizeBefore) {
                if (rows.size >= PAGE_SIZE) throw ApiFailure.HistoryCannotBePaged(boundary.orEmpty())
                return collected.values.toList()
            }
            boundary = date(rows.last())
        }
    }

    private suspend inline fun <reified T> get(path: String, vararg query: Pair<String, String?>): T {
        val url = baseUrl.newBuilder().addPathSegments(path)
        for ((name, value) in query) {
            if (value != null) url.addQueryParameter(name, value)
        }
        val request =
            Request.Builder()
                .url(url.build())
                .header("Authorization", "Bearer $apiKey")
                .build()
        return decode(execute(request))
    }

    private inline fun <reified T> decode(body: String): T =
        try {
            json.decodeFromString<T>(body)
        } catch (error: SerializationException) {
            throw ApiFailure.UnreadableResponse(error)
        }

    private suspend fun execute(request: Request): String =
        runInterruptible(Dispatchers.IO) {
            try {
                client.newCall(request).execute().use { response ->
                    when {
                        response.code == 401 -> throw ApiFailure.Unauthorized()
                        !response.isSuccessful -> throw ApiFailure.Rejected(response.code)
                        else -> response.body.string()
                    }
                }
            } catch (error: IOException) {
                throw ApiFailure.Unreachable(error)
            }
        }
}
