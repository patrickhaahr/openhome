package com.patrickhaahr.openhome.e2e

import java.net.HttpURLConnection
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.double
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import kotlinx.serialization.json.put
import mockwebserver3.Dispatcher
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest
import mockwebserver3.SocketEffect

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
    val clientRevision: Long? = null,
)

data class FixtureBodyWeight(val id: Long, val date: String, val weightKg: Double)

data class FixtureExercise(
    val id: Long,
    val name: String,
    val category: String = "calisthenics",
    val muscleGroup: String? = null,
    val equipment: String? = null,
)

data class FixtureProfile(
    val heightCm: Double? = null,
    val sex: String? = null,
    val hrMaxBpm: Int? = null,
    val birthDate: String? = null,
)

/**
 * The app's external boundary: an HTTP server speaking the Axum fitness contract, including its
 * `Authorization` check and its list paging (`limit` defaults to 100 and clamps to 1..500, Workouts
 * run newest first with an inclusive `to`, Body Weight oldest first with an inclusive `from`) and
 * Workout and Exercise creates with a `request_id`: a repeat answers 200 with the same record, and the
 * same `request_id` with changed arguments is a 409. Exercise names are unique, also a 409. Workout
 * edits replace the entries they state, keep a name or notes they leave out, and apply only when their
 * `client_revision` is newer, otherwise answering the Workout as it is. Its Workouts and Exercises are
 * what a test reads back independently.
 */
class FakeAxum(
    exercises: Map<Long, String> = emptyMap(),
    workouts: List<FixtureWorkout> = emptyList(),
    private val bodyWeight: List<FixtureBodyWeight> = emptyList(),
    private val profile: FixtureProfile = FixtureProfile(),
    exerciseCategories: Map<Long, String> = emptyMap(),
    exerciseDetails: List<FixtureExercise> = emptyList(),
) : AutoCloseable {
    private val server = MockWebServer()
    private val detailRequests = AtomicInteger()
    private val blockedDetails = CountDownLatch(1)
    private val workouts = CopyOnWriteArrayList(workouts)
    private val receipts = HashMap<String, Pair<Long, JsonObject>>()
    private val deliveries = CopyOnWriteArrayList<String>()
    private val exercises =
        CopyOnWriteArrayList(
            exercises.map { (id, name) -> FixtureExercise(id, name, exerciseCategories[id] ?: "calisthenics") } + exerciseDetails,
        )
    private val exerciseReceipts = HashMap<String, Pair<Long, JsonObject>>()
    private val exerciseDeliveries = CopyOnWriteArrayList<String>()
    private val createOrder = CopyOnWriteArrayList<String>()
    private val editDeliveries = CopyOnWriteArrayList<Long>()

    @Volatile
    private var workoutEdits = WorkoutEdits.ANSWERED

    @Volatile
    private var exerciseCreates = ExerciseCreates.ANSWERED

    @Volatile
    private var workoutCreates = WorkoutCreates.ANSWERED

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
        workoutCreates = WorkoutCreates.ANSWERED
        exerciseCreates = ExerciseCreates.ANSWERED
        workoutEdits = WorkoutEdits.ANSWERED
    }

    /** Commit each Workout edit, then cut the connection before answering. */
    fun loseEditResponses() {
        workoutEdits = WorkoutEdits.COMMITTED_RESPONSE_LOST
    }

    /** Another client deletes a Workout this app has downloaded. */
    fun deleteWorkout(id: Long) {
        workouts.removeAll { it.id == id }
    }

    /** The `client_revision` of every Workout edit received, in order, including lost ones. */
    fun editDeliveries(): List<Long> = editDeliveries.toList()

    /** Commit each Exercise create, then cut the connection before answering. */
    fun loseExerciseResponses() {
        exerciseCreates = ExerciseCreates.COMMITTED_RESPONSE_LOST
    }

    /** Another client adds to the Library, or renames in it, after this app's Setup Flow. */
    fun putExercise(exercise: FixtureExercise) {
        exercises.removeAll { it.id == exercise.id }
        exercises += exercise
    }

    /** The stored Exercises, read independently of anything the app reports. */
    fun storedExercises(): List<FixtureExercise> = exercises.toList()

    /** The `request_id` of every Exercise create received, in order, including lost and refused ones. */
    fun exerciseDeliveries(): List<String> = exerciseDeliveries.toList()

    /** What reached Axum and committed, in order: "exercise:<name>", "workout:<name>" or "edit:<name>". */
    fun commitOrder(): List<String> = createOrder.toList()

    /** Commit each Workout create, then cut the connection before answering. */
    fun loseWorkoutResponses() {
        workoutCreates = WorkoutCreates.COMMITTED_RESPONSE_LOST
    }

    /** Cut the connection on each Workout create without committing it, as an unreachable API would. */
    fun dropWorkoutRequests() {
        workoutCreates = WorkoutCreates.DROPPED
    }

    /** Answer each Workout create with a 500 and commit nothing. */
    fun failWorkoutCreates() {
        workoutCreates = WorkoutCreates.SERVER_ERROR
    }

    /** Answer each Workout create with a 422 and commit nothing. */
    fun refuseWorkoutCreates() {
        workoutCreates = WorkoutCreates.REFUSED
    }

    /** The stored Workouts, read independently of anything the app reports. */
    fun storedWorkouts(): List<FixtureWorkout> = workouts.toList()

    /** The `request_id` of every Workout create received, in order, including lost and refused ones. */
    fun workoutDeliveries(): List<String> = deliveries.toList()

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
            path == "/api/exercises" && request.method == "POST" -> createExercise(request)
            path == "/api/exercises" -> ok(exercisesJson())
            path == "/api/profile" -> ok(profileJson())
            path == "/api/workouts" && request.method == "POST" -> createWorkout(request)
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
            path.startsWith("/api/workouts/") && request.method == "PATCH" ->
                editWorkout(path.substringAfterLast('/').toLong(), request)
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

    private fun createWorkout(request: RecordedRequest): MockResponse {
        val body = Json.parseToJsonElement(checkNotNull(request.body).utf8()).jsonObject
        val requestId = checkNotNull(body["request_id"]).jsonPrimitive.content
        deliveries += requestId
        return when (workoutCreates) {
            WorkoutCreates.DROPPED -> disconnected()
            WorkoutCreates.SERVER_ERROR -> error(HttpURLConnection.HTTP_INTERNAL_ERROR, "An internal error occurred")
            WorkoutCreates.REFUSED -> error(UNPROCESSABLE_ENTITY, "Exercise not found")
            WorkoutCreates.ANSWERED -> commitWorkout(requestId, body)
            WorkoutCreates.COMMITTED_RESPONSE_LOST -> {
                commitWorkout(requestId, body)
                disconnected()
            }
        }
    }

    private fun editWorkout(id: Long, request: RecordedRequest): MockResponse {
        val body = Json.parseToJsonElement(checkNotNull(request.body).utf8()).jsonObject
        val revision = checkNotNull(body["client_revision"]).jsonPrimitive.long
        editDeliveries += revision
        val answer = commitEdit(id, revision, body)
        return if (workoutEdits == WorkoutEdits.COMMITTED_RESPONSE_LOST) disconnected() else answer
    }

    private fun commitEdit(id: Long, revision: Long, body: JsonObject): MockResponse =
        synchronized(receipts) {
            val current =
                workouts.firstOrNull { it.id == id }
                    ?: return error(HttpURLConnection.HTTP_NOT_FOUND, "Workout with id $id not found")
            if ((current.clientRevision ?: 0) >= revision) return ok(detailJson(current))
            val edited =
                body.toFixtureWorkout(id).copy(
                    name = if ("name" in body) body.text("name") else current.name,
                    notes = if ("notes" in body) body.text("notes") else current.notes,
                    clientRevision = revision,
                )
            workouts[workouts.indexOf(current)] = edited
            createOrder += "edit:${edited.name}"
            ok(detailJson(edited))
        }

    private fun createExercise(request: RecordedRequest): MockResponse {
        val body = Json.parseToJsonElement(checkNotNull(request.body).utf8()).jsonObject
        val requestId = checkNotNull(body["request_id"]).jsonPrimitive.content
        exerciseDeliveries += requestId
        val answer = commitExercise(requestId, body)
        return if (exerciseCreates == ExerciseCreates.COMMITTED_RESPONSE_LOST) disconnected() else answer
    }

    private fun commitExercise(requestId: String, body: JsonObject): MockResponse =
        synchronized(exerciseReceipts) {
            val arguments = JsonObject(body - "request_id")
            val receipt = exerciseReceipts[requestId]
            if (receipt != null) {
                if (receipt.second != arguments) {
                    return error(HttpURLConnection.HTTP_CONFLICT, "request_id was already used for a different Exercise")
                }
                return ok(exerciseJson(exercises.first { it.id == receipt.first }))
            }
            val name = arguments.getValue("name").jsonPrimitive.content
            if (exercises.any { it.name == name }) {
                return error(HttpURLConnection.HTTP_CONFLICT, "Exercise '$name' already exists")
            }
            val exercise =
                FixtureExercise(
                    id = (exercises.maxOfOrNull { it.id } ?: 0) + 1,
                    name = name,
                    category = arguments.getValue("category").jsonPrimitive.content,
                    muscleGroup = arguments.text("muscle_group"),
                    equipment = arguments.text("equipment"),
                )
            exercises += exercise
            exerciseReceipts[requestId] = exercise.id to arguments
            createOrder += "exercise:$name"
            MockResponse.Builder()
                .code(HttpURLConnection.HTTP_CREATED)
                .addHeader("Content-Type", "application/json")
                .body(exerciseJson(exercise).toString())
                .build()
        }

    private fun commitWorkout(requestId: String, body: JsonObject): MockResponse =
        synchronized(receipts) {
            val arguments = JsonObject(body - "request_id")
            val receipt = receipts[requestId]
            if (receipt != null) {
                if (receipt.second != arguments) {
                    return error(HttpURLConnection.HTTP_CONFLICT, "request_id was already used for a different Workout")
                }
                return ok(detailJson(workouts.first { it.id == receipt.first }))
            }
            val workout = arguments.toFixtureWorkout(id = (workouts.maxOfOrNull { it.id } ?: 0) + 1)
            workouts += workout
            receipts[requestId] = workout.id to arguments
            createOrder += "workout:${workout.name}"
            MockResponse.Builder()
                .code(HttpURLConnection.HTTP_CREATED)
                .addHeader("Content-Type", "application/json")
                .body(detailJson(workout).toString())
                .build()
        }

    private fun JsonObject.toFixtureWorkout(id: Long) =
        FixtureWorkout(
            id = id,
            date = getValue("date").jsonPrimitive.content,
            name = text("name"),
            notes = text("notes"),
            entries =
                getValue("exercises").jsonArray.map { element ->
                    val entry = element.jsonObject
                    FixtureEntry(
                        exerciseId = entry.getValue("exercise_id").jsonPrimitive.long,
                        notes = entry.text("notes"),
                        sets =
                            entry.getValue("sets").jsonArray.map { setElement ->
                                val set = setElement.jsonObject
                                FixtureSet(
                                    setNumber = set.getValue("set_number").jsonPrimitive.int,
                                    reps = set.number("reps")?.int,
                                    weightKg = set.number("weight_kg")?.double,
                                    durationSeconds = set.number("duration_seconds")?.int,
                                    rpe = set.number("rpe")?.int,
                                    notes = set.text("notes"),
                                )
                            },
                    )
                },
        )

    private fun JsonObject.text(key: String): String? = number(key)?.content

    private fun JsonObject.number(key: String): JsonPrimitive? =
        (this[key] as? JsonPrimitive)?.takeUnless { it is JsonNull }

    private fun disconnected(): MockResponse =
        MockResponse.Builder().onResponseStart(SocketEffect.CloseSocket()).build()

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

    private fun exercisesJson() = JsonArray(exercises.map(::exerciseJson))

    private fun exerciseJson(exercise: FixtureExercise) =
        buildJsonObject {
            put("id", exercise.id)
            put("name", exercise.name)
            put("category", exercise.category)
            put("muscle_group", exercise.muscleGroup)
            put("equipment", exercise.equipment)
        }

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
            workout.clientRevision?.let { put("client_revision", it) }
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
                                        put("name", JsonPrimitive(exercises.first { it.id == entry.exerciseId }.name))
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

private const val UNPROCESSABLE_ENTITY = 422

private enum class WorkoutEdits {
    ANSWERED,
    COMMITTED_RESPONSE_LOST,
}

private enum class ExerciseCreates {
    ANSWERED,
    COMMITTED_RESPONSE_LOST,
}

private enum class WorkoutCreates {
    ANSWERED,
    COMMITTED_RESPONSE_LOST,
    DROPPED,
    SERVER_ERROR,
    REFUSED,
}
