package com.patrickhaahr.openhome.e2e

import java.net.HttpURLConnection
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import mockwebserver3.Dispatcher
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest

const val API_KEY = "e2e-api-key"

data class FixtureSet(
    val setNumber: Int,
    val reps: Int? = null,
    val weightKg: Double? = null,
    val durationSeconds: Int? = null,
    val rpe: Int? = null,
    val notes: String? = null,
)

data class FixtureEntry(val exerciseId: Long, val sets: List<FixtureSet>, val notes: String? = null)

data class FixtureWorkout(
    val id: Long,
    val date: String,
    val name: String?,
    val notes: String? = null,
    val entries: List<FixtureEntry> = emptyList(),
)

data class FixtureBodyWeight(val id: Long, val date: String, val weightKg: Double)

data class FixtureProfile(
    val heightCm: Double? = null,
    val sex: String? = null,
    val hrMaxBpm: Int? = null,
    val birthDate: String? = null,
)

/**
 * The app's external boundary: an HTTP server speaking the Axum fitness contract, including its
 * `Authorization` check and its list paging (`limit` defaults to 100 and clamps to 1..500, Workouts
 * run newest first with an inclusive `to`, Body Weight oldest first with an inclusive `from`).
 */
class FakeAxum(
    private val exercises: Map<Long, String> = emptyMap(),
    private val workouts: List<FixtureWorkout> = emptyList(),
    private val bodyWeight: List<FixtureBodyWeight> = emptyList(),
    private val profile: FixtureProfile = FixtureProfile(),
) : AutoCloseable {
    private val server = MockWebServer()
    private val detailRequests = AtomicInteger()
    private val blockedDetails = CountDownLatch(1)

    @Volatile
    private var failDetailsAfter: Int? = null

    @Volatile
    private var releaseBlockedDetails: CountDownLatch? = null

    init {
        server.dispatcher =
            object : Dispatcher() {
                override fun dispatch(request: RecordedRequest): MockResponse = respond(request)
            }
        server.start()
    }

    val baseUrl: String
        get() = "http://127.0.0.1:${server.port}"

    /** Answer later Workout detail reads with an API error, as a server failing mid-download would. */
    fun failWorkoutDetailsAfter(successes: Int) {
        failDetailsAfter = successes
    }

    fun healthy() {
        failDetailsAfter = null
    }

    /** Hang Workout detail reads, so the test can act while the app is still downloading. */
    fun holdWorkoutDetails(): CountDownLatch =
        CountDownLatch(1).also { releaseBlockedDetails = it }

    fun awaitHeldDetailRequest() {
        check(blockedDetails.await(20, TimeUnit.SECONDS)) { "The app never requested a Workout detail" }
    }

    override fun close() {
        releaseBlockedDetails?.countDown()
        server.close()
    }

    private fun respond(request: RecordedRequest): MockResponse {
        if (request.headers["Authorization"] != "Bearer $API_KEY") {
            return error(HttpURLConnection.HTTP_UNAUTHORIZED, "Invalid API key")
        }
        val url = request.url
        val path = url.encodedPath
        return when {
            path == "/api/health" -> ok(buildJsonObject { put("status", "ok") })
            path == "/api/exercises" -> ok(exercisesJson())
            path == "/api/profile" -> ok(profileJson())
            path == "/api/workouts" -> {
                val to = url.queryParameter("to")
                ok(
                    JsonArray(
                        workouts
                            .filter { to == null || it.date <= to }
                            .sortedWith(compareByDescending<FixtureWorkout> { it.date }.thenByDescending { it.id })
                            .take(pageLimit(url.queryParameter("limit")))
                            .map(::summaryJson),
                    ),
                )
            }
            path.startsWith("/api/workouts/") -> workoutDetail(path.substringAfterLast('/').toLong())
            path == "/api/body_weight" -> {
                val from = url.queryParameter("from")
                ok(
                    JsonArray(
                        bodyWeight
                            .filter { from == null || it.date >= from }
                            .sortedBy { it.date }
                            .take(pageLimit(url.queryParameter("limit")))
                            .map {
                                buildJsonObject {
                                    put("id", it.id)
                                    put("date", it.date)
                                    put("weight_kg", it.weightKg)
                                }
                            },
                    ),
                )
            }
            else -> error(HttpURLConnection.HTTP_NOT_FOUND, "Not found")
        }
    }

    private fun workoutDetail(id: Long): MockResponse {
        releaseBlockedDetails?.let {
            blockedDetails.countDown()
            it.await(60, TimeUnit.SECONDS)
        }
        val limit = failDetailsAfter
        if (limit != null && detailRequests.incrementAndGet() > limit) {
            return error(HttpURLConnection.HTTP_INTERNAL_ERROR, "An internal error occurred")
        }
        val workout =
            workouts.firstOrNull { it.id == id }
                ?: return error(HttpURLConnection.HTTP_NOT_FOUND, "Workout with id $id not found")
        return ok(detailJson(workout))
    }

    private fun pageLimit(value: String?): Int = (value?.toIntOrNull() ?: 100).coerceIn(1, 500)

    private fun ok(json: Any): MockResponse =
        MockResponse.Builder()
            .code(HttpURLConnection.HTTP_OK)
            .addHeader("Content-Type", "application/json")
            .body(json.toString())
            .build()

    private fun error(status: Int, message: String): MockResponse =
        MockResponse.Builder()
            .code(status)
            .addHeader("Content-Type", "application/json")
            .body(
                buildJsonObject {
                    put("error", message)
                    put("status", status)
                }.toString(),
            )
            .build()

    private fun exercisesJson() =
        JsonArray(
            exercises.entries.map { (id, name) ->
                buildJsonObject {
                    put("id", id)
                    put("name", name)
                    put("category", "calisthenics")
                    put("muscle_group", JsonNull)
                    put("equipment", JsonNull)
                }
            },
        )

    private fun profileJson() =
        buildJsonObject {
            put("height_cm", profile.heightCm)
            put("sex", profile.sex)
            put("hr_max_bpm", profile.hrMaxBpm)
            put("birth_date", profile.birthDate)
        }

    private fun summaryJson(workout: FixtureWorkout) =
        buildJsonObject {
            put("id", workout.id)
            put("date", workout.date)
            put("name", workout.name)
        }

    private fun detailJson(workout: FixtureWorkout): JsonObject =
        buildJsonObject {
            put("id", workout.id)
            put("date", workout.date)
            put("name", workout.name)
            put("notes", workout.notes)
            put(
                "exercises",
                buildJsonArray {
                    workout.entries.forEachIndexed { index, entry ->
                        add(
                            buildJsonObject {
                                put("id", workout.id * 100 + index)
                                put("order_index", index)
                                put("notes", entry.notes)
                                put(
                                    "exercise",
                                    buildJsonObject {
                                        put("id", entry.exerciseId)
                                        put("name", JsonPrimitive(exercises.getValue(entry.exerciseId)))
                                        put("category", "calisthenics")
                                    },
                                )
                                put("sets", JsonArray(entry.sets.map(::setJson)))
                            },
                        )
                    }
                },
            )
        }

    private fun setJson(set: FixtureSet) =
        buildJsonObject {
            put("id", set.setNumber)
            put("set_number", set.setNumber)
            put("reps", set.reps)
            put("weight_kg", set.weightKg)
            put("duration_seconds", set.durationSeconds)
            put("rpe", set.rpe)
            put("notes", set.notes)
        }
}
