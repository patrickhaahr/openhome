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
    val clientRevision: Long? = null,
)

/**
 * The app's external boundary: an HTTP server speaking the Axum fitness contract, including its
 * `Authorization` check and its list paging (`limit` defaults to 100 and clamps to 1..500, Workouts
 * run newest first with an inclusive `to`, Body Weight oldest first with an inclusive `from`) and
 * Workout and Exercise creates with a `request_id`: a repeat answers 200 with the same record, and the
 * same `request_id` with changed arguments is a 409. Exercise names are unique, also a 409. Workout
 * edits replace the entries they state, keep a name or notes they leave out, and apply only when their
 * `client_revision` is newer, otherwise answering the Workout as it is. Body Weight records follow the
 * same `request_id` rule, and a date recorded under any other is a 409. Profile edits keep what they
 * leave out, clear what they send as null and follow the same `client_revision` rule. Its stored records
 * are what a test reads back independently.
 */
class FakeAxum(
    exercises: Map<Long, String> = emptyMap(),
    workouts: List<FixtureWorkout> = emptyList(),
    bodyWeight: List<FixtureBodyWeight> = emptyList(),
    profile: FixtureProfile = FixtureProfile(),
    exerciseCategories: Map<Long, String> = emptyMap(),
    exerciseDetails: List<FixtureExercise> = emptyList(),
) : AutoCloseable {
    @Volatile var tvCommands: List<String> = listOf("power", "input", "mute", "volume-up", "volume-down", "up", "left", "ok", "right", "down", "back", "home", "exit")
    @Volatile var tvStatusCode: Int = 200
    @Volatile var tvCommandCode: Int = 200
    @Volatile var loseTvResponse: Boolean = false
    private val tvRequests = CopyOnWriteArrayList<JsonObject>()
    fun tvRequests(): List<JsonObject> = tvRequests.toList()

    @Volatile var speakerCommands = listOf("power", "mute", "volume-down", "volume-up", "bluetooth", "optical")
    @Volatile var speakerCommandCode = 200
    @Volatile var lightCommandCode = 200
    @Volatile var loseSpeakerResponse = false
    @Volatile var loseLightResponse = false
    private val speakerRequests = CopyOnWriteArrayList<JsonObject>()
    private val lightRequests = CopyOnWriteArrayList<Pair<String, String>>()
    fun speakerRequests(): List<JsonObject> = speakerRequests.toList()
    fun lightRequests(): List<Pair<String, String>> = lightRequests.toList()

    @Volatile var backlightStatus: JsonObject = Json.parseToJsonElement("""
        {
          "effects":[{"id":"aurora","name":"Aurora","description":"Cool waves."}],
          "setup":"ready","reachability":"reachable",
          "intended":{"mode":"color","rgb":{"red":20,"green":40,"blue":60},"brightness":35},
          "acknowledged":{"mode":"tv-sync","brightness":70},
          "observed":{"power":false,"mode":"other","brightness":12,"observed_at":"2026-01-01T12:00:00Z"},
          "stale":true,"command":"failed","recovery":"retrying",
          "bedtime":{"enabled":true,"start":"22:30","end":"06:00","brightness":8},
          "control_source":"manual","override_expires_at":"2026-11-07T05:00:00Z",
          "next_transition":"2026-11-07T05:00:00Z"
        }
    """.trimIndent()).jsonObject
    @Volatile var backlightCode = 200
    @Volatile var loseBacklightResponse = false
    @Volatile var backlightDelayMillis = 0L
    private val backlightRequests = CopyOnWriteArrayList<Pair<String, String>>()
    private val backlightMethods = CopyOnWriteArrayList<String>()
    fun backlightMethods(): List<String> = backlightMethods.toList()
    private val automaticRequest = CountDownLatch(1)
    fun awaitAutomaticRequest() {
        check(automaticRequest.await(15, TimeUnit.SECONDS)) { "Automatic never reached Axum" }
    }
    fun backlightRequests(): List<Pair<String, String>> = backlightRequests.toList()

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
    private val bodyWeight = CopyOnWriteArrayList(bodyWeight)
    private val bodyWeightReceipts = HashMap<String, Pair<Long, JsonObject>>()
    private val bodyWeightDeliveries = CopyOnWriteArrayList<String>()
    private val profileDeliveries = CopyOnWriteArrayList<Long>()

    @Volatile
    private var profile = profile

    @Volatile
    private var losingBodyWeightResponses = false

    @Volatile
    private var losingProfileResponses = false

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
        losingBodyWeightResponses = false
        losingProfileResponses = false
    }

    /** Commit each Body Weight record, then cut the connection before answering. */
    fun loseBodyWeightResponses() {
        losingBodyWeightResponses = true
    }

    /** Commit each Profile edit, then cut the connection before answering. */
    fun loseProfileResponses() {
        losingProfileResponses = true
    }

    /** Another client records a Body Weight after this app's Setup Flow. */
    fun putBodyWeight(date: String, weightKg: Double) {
        bodyWeight += FixtureBodyWeight((bodyWeight.maxOfOrNull { it.id } ?: 0) + 1, date, weightKg)
    }

    /** The stored Body Weight records, oldest first, read independently of anything the app reports. */
    fun storedBodyWeight(): List<FixtureBodyWeight> = bodyWeight.sortedBy { it.date }

    /** The stored Profile, read independently of anything the app reports. */
    fun storedProfile(): FixtureProfile = profile

    /** The `request_id` of every Body Weight record received, in order, including lost and refused ones. */
    fun bodyWeightDeliveries(): List<String> = bodyWeightDeliveries.toList()

    /** The `client_revision` of every Profile edit received, in order, including lost ones. */
    fun profileDeliveries(): List<Long> = profileDeliveries.toList()

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
            path == "/api/ir" -> if (tvStatusCode != 200) error(tvStatusCode, "IR unavailable") else ok(
                buildJsonObject { put("remotes", buildJsonObject {
                    put("edifier", JsonArray(speakerCommands.map(::JsonPrimitive)))
                    put("lgtv", JsonArray(tvCommands.map(::JsonPrimitive)))
                }) },
            )
            path == "/api/ir/lgtv" && request.method == "POST" -> {
                tvRequests += Json.parseToJsonElement(checkNotNull(request.body).utf8()).jsonObject
                when {
                    loseTvResponse -> disconnected()
                    tvCommandCode != 200 -> error(tvCommandCode, "IR unavailable")
                    else -> ok(buildJsonObject { put("message", "Sent command") })
                }
            }
            path == "/api/ir/edifier" && request.method == "POST" -> {
                speakerRequests += Json.parseToJsonElement(checkNotNull(request.body).utf8()).jsonObject
                when {
                    loseSpeakerResponse -> disconnected()
                    speakerCommandCode != 200 -> error(speakerCommandCode, "Speaker unavailable")
                    else -> ok(buildJsonObject { put("message", "Sent command") })
                }
            }
            path in listOf("/api/lights/on", "/api/lights/off") && request.method == "POST" -> {
                lightRequests += path to (request.body?.utf8() ?: "")
                when {
                    loseLightResponse -> disconnected()
                    lightCommandCode != 200 -> error(lightCommandCode, "SwitchBot service is not configured")
                    else -> ok(buildJsonObject { put("message", "Sent command") })
                }
            }
            path.startsWith("/api/backlight") -> {
                if (request.method != "GET") {
                    backlightRequests += path to (request.body?.utf8() ?: "")
                    backlightMethods += request.method.orEmpty()
                }
                if (path == "/api/backlight/automatic") automaticRequest.countDown()
                if (backlightDelayMillis > 0) Thread.sleep(backlightDelayMillis)
                when {
                    loseBacklightResponse && path != "/api/backlight" -> disconnected()
                    backlightCode != 200 -> error(backlightCode, "Backlight unavailable")
                    else -> ok(backlightStatus)
                }
            }
            path == "/api/health" -> ok(buildJsonObject { put("status", "ok") })
            path == "/api/exercises" && request.method == "POST" -> createExercise(request)
            path == "/api/exercises" -> ok(exercisesJson())
            path == "/api/profile" && request.method == "PATCH" -> editProfile(request)
            path == "/api/profile" -> ok(profileJson())
            path == "/api/body_weight" && request.method == "POST" -> createBodyWeight(request)
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
                val to = url.queryParameter("to")
                ok(
                    JsonArray(
                        bodyWeight
                            .filter { (from == null || it.date >= from) && (to == null || it.date <= to) }
                            .sortedBy { it.date }
                            .take(pageLimit(url.queryParameter("limit")))
                            .map(::bodyWeightJson),
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

    private fun createBodyWeight(request: RecordedRequest): MockResponse {
        val body = Json.parseToJsonElement(checkNotNull(request.body).utf8()).jsonObject
        val requestId = checkNotNull(body["request_id"]).jsonPrimitive.content
        bodyWeightDeliveries += requestId
        val answer = commitBodyWeight(requestId, body)
        return if (losingBodyWeightResponses) disconnected() else answer
    }

    private fun commitBodyWeight(requestId: String, body: JsonObject): MockResponse =
        synchronized(bodyWeightReceipts) {
            val arguments = JsonObject(body - "request_id")
            val receipt = bodyWeightReceipts[requestId]
            if (receipt != null) {
                if (receipt.second != arguments) {
                    return error(HttpURLConnection.HTTP_CONFLICT, "request_id was already used for a different Body Weight")
                }
                return ok(bodyWeightJson(bodyWeight.first { it.id == receipt.first }))
            }
            val date = arguments.getValue("date").jsonPrimitive.content
            if (bodyWeight.any { it.date == date }) {
                return error(HttpURLConnection.HTTP_CONFLICT, "Body weight for date '$date' already recorded")
            }
            val entry =
                FixtureBodyWeight(
                    id = (bodyWeight.maxOfOrNull { it.id } ?: 0) + 1,
                    date = date,
                    weightKg = arguments.getValue("weight_kg").jsonPrimitive.double,
                )
            bodyWeight += entry
            bodyWeightReceipts[requestId] = entry.id to arguments
            MockResponse.Builder()
                .code(HttpURLConnection.HTTP_CREATED)
                .addHeader("Content-Type", "application/json")
                .body(bodyWeightJson(entry).toString())
                .build()
        }

    private fun editProfile(request: RecordedRequest): MockResponse {
        val body = Json.parseToJsonElement(checkNotNull(request.body).utf8()).jsonObject
        val revision = checkNotNull(body["client_revision"]).jsonPrimitive.long
        profileDeliveries += revision
        synchronized(profileDeliveries) {
            val current = profile
            if ((current.clientRevision ?: 0) < revision) {
                profile =
                    FixtureProfile(
                        heightCm = if ("height_cm" in body) body.number("height_cm")?.double else current.heightCm,
                        sex = if ("sex" in body) body.text("sex") else current.sex,
                        hrMaxBpm = if ("hr_max_bpm" in body) body.number("hr_max_bpm")?.int else current.hrMaxBpm,
                        birthDate = if ("birth_date" in body) body.text("birth_date") else current.birthDate,
                        clientRevision = revision,
                    )
            }
        }
        return if (losingProfileResponses) disconnected() else ok(profileJson())
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

    private fun bodyWeightJson(entry: FixtureBodyWeight) =
        buildJsonObject {
            put("id", entry.id)
            put("date", entry.date)
            put("weight_kg", entry.weightKg)
        }

    private fun profileJson() =
        buildJsonObject {
            put("height_cm", profile.heightCm)
            put("sex", profile.sex)
            put("hr_max_bpm", profile.hrMaxBpm)
            put("birth_date", profile.birthDate)
            profile.clientRevision?.let { put("client_revision", it) }
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
