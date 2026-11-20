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

private const val PAGE_SIZE = 500

private val JSON_MEDIA_TYPE = "application/json".toMediaType()

sealed class ApiFailure(message: String, cause: Throwable? = null) : Exception(message, cause) {
    class Unreachable(cause: IOException) : ApiFailure("Axum API unreachable", cause)

    class Unauthorized : ApiFailure("Axum API rejected the API Key")

    class Rejected(val status: Int) : ApiFailure("Axum API answered HTTP $status")

    class UnreadableResponse(cause: Throwable? = null) : ApiFailure("Unreadable Axum API response", cause)

    class HistoryCannotBePaged(val date: String) : ApiFailure("Cannot page past $date")
}

fun newOpenHomeHttpClient(): OkHttpClient =
    OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .build()

class OpenHomeApi(
    private val baseUrl: HttpUrl,
    private val apiKey: String,
    private val client: OkHttpClient,
) {
    private val json = Json {
        ignoreUnknownKeys = true
        classDiscriminator = "mode"
    }

    suspend fun checkConnection() {
        val health =
            try {
                get<HealthResponse>("api/health")
            } catch (failure: ApiFailure.Rejected) {
                if (failure.status == 404) throw ApiFailure.UnreadableResponse(failure) else throw failure
            }
        if (health.status != "ok") throw ApiFailure.UnreadableResponse()
    }

    suspend fun backlightStatus(): BacklightStatus = get("api/backlight")

    suspend fun refreshBacklight(): BacklightStatus = post("api/backlight/refresh", "")

    suspend fun selectBacklight(selection: BacklightSelection): BacklightStatus =
        send("PUT", "api/backlight/selection", json.encodeToString(selection))

    suspend fun automaticBacklight(): BacklightStatus = post("api/backlight/automatic", "")

    suspend fun updateBacklightBedtime(settings: BacklightBedtimeUpdate): BacklightStatus =
        send("PATCH", "api/backlight/bedtime", json.encodeToString(settings))

    suspend fun saveBacklightProfile(source: BacklightProfileSource): BacklightStatus =
        post("api/backlight/profile", json.encodeToString(BacklightProfileRequest(source)))

    suspend fun irStatus(): ApiIrStatus = get("api/ir")

    suspend fun sendTvCommand(command: String): ApiIrAcknowledgement =
        post("api/ir/lgtv", json.encodeToString(ApiIrCommand(command)))

    suspend fun sendSpeakerCommand(command: String): ApiIrAcknowledgement =
        post("api/ir/edifier", json.encodeToString(ApiIrCommand(command)))

    suspend fun sendLightCommand(command: LightCommand): ApiIrAcknowledgement =
        post("api/lights/${command.path}", "")

    suspend fun exercises(): List<ApiExercise> = get("api/exercises")

    suspend fun workoutDetail(id: Long): ApiWorkoutDetail = get("api/workouts/$id")

    suspend fun profile(): ApiProfile = get("api/profile")

    suspend fun createWorkout(workout: ApiWorkoutCreate): ApiWorkoutCreated =
        post("api/workouts", json.encodeToString(workout))

    suspend fun createExercise(exercise: ApiExerciseCreate): ApiExercise =
        post("api/exercises", json.encodeToString(exercise))

    suspend fun editWorkout(id: Long, edit: ApiWorkoutEdit): ApiWorkoutCreated =
        send("PATCH", "api/workouts/$id", json.encodeToString(edit))

    suspend fun createBodyWeight(entry: ApiBodyWeightCreate): ApiBodyWeight =
        post("api/body_weight", json.encodeToString(entry))

    suspend fun editProfile(edit: ApiProfileEdit): ApiProfile =
        send("PATCH", "api/profile", json.encodeToString(edit))

    suspend fun bodyWeightOn(date: String): ApiBodyWeight? =
        get<List<ApiBodyWeight>>("api/body_weight", "from" to date, "to" to date, "limit" to "1").firstOrNull()

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

    suspend fun allWorkoutSummaries(): List<ApiWorkoutSummary> =
        collectAllPages(id = ApiWorkoutSummary::id, date = ApiWorkoutSummary::date) { boundary ->
            get("api/workouts", "limit" to PAGE_SIZE.toString(), "to" to boundary)
        }

    suspend fun allBodyWeight(): List<ApiBodyWeight> =
        collectAllPages(id = ApiBodyWeight::id, date = ApiBodyWeight::date) { boundary ->
            get("api/body_weight", "limit" to PAGE_SIZE.toString(), "from" to boundary)
        }

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
