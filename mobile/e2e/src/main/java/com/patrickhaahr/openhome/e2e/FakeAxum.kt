package com.patrickhaahr.openhome.e2e

import java.net.HttpURLConnection
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import mockwebserver3.Dispatcher
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest


const val API_KEY = "e2e-api-key"

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

    @Volatile private var commandGate: CountDownLatch? = null

    fun holdCommands(): CountDownLatch = CountDownLatch(1).also { commandGate = it }

    private fun awaitCommandGate() {
        commandGate?.await(60, TimeUnit.SECONDS)
    }
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
    internal val detailRequests = AtomicInteger()
    internal val blockedDetails = CountDownLatch(1)
    internal val workouts = CopyOnWriteArrayList(workouts)
    internal val receipts = HashMap<String, Pair<Long, JsonObject>>()
    internal val deliveries = CopyOnWriteArrayList<String>()
    internal val exercises =
        CopyOnWriteArrayList(
            exercises.map { (id, name) -> FixtureExercise(id, name, exerciseCategories[id] ?: "calisthenics") } + exerciseDetails,
        )
    internal val exerciseReceipts = HashMap<String, Pair<Long, JsonObject>>()
    internal val exerciseDeliveries = CopyOnWriteArrayList<String>()
    internal val createOrder = CopyOnWriteArrayList<String>()
    internal val editDeliveries = CopyOnWriteArrayList<Long>()
    internal val bodyWeight = CopyOnWriteArrayList(bodyWeight)
    internal val bodyWeightReceipts = HashMap<String, Pair<Long, JsonObject>>()
    internal val bodyWeightDeliveries = CopyOnWriteArrayList<String>()
    internal val profileDeliveries = CopyOnWriteArrayList<Long>()

    @Volatile
    internal var profile = profile

    @Volatile
    internal var losingBodyWeightResponses = false

    @Volatile
    internal var losingProfileResponses = false

    @Volatile
    internal var workoutEdits = WorkoutEdits.ANSWERED

    @Volatile
    internal var exerciseCreates = ExerciseCreates.ANSWERED

    @Volatile
    internal var workoutCreates = WorkoutCreates.ANSWERED

    @Volatile
    internal var failDetailsAfter: Int? = null

    @Volatile
    internal var releaseBlockedDetails: CountDownLatch? = null

    init {
        server.dispatcher =
            object : Dispatcher() {
                override fun dispatch(request: RecordedRequest): MockResponse = respond(request)
            }
        server.start()
    }

    val baseUrl: String
        get() = "http://127.0.0.1:${server.port}"

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

    fun loseBodyWeightResponses() {
        losingBodyWeightResponses = true
    }

    fun loseProfileResponses() {
        losingProfileResponses = true
    }

    fun putBodyWeight(date: String, weightKg: Double) {
        bodyWeight += FixtureBodyWeight((bodyWeight.maxOfOrNull { it.id } ?: 0) + 1, date, weightKg)
    }

    fun storedBodyWeight(): List<FixtureBodyWeight> = bodyWeight.sortedBy { it.date }

    fun storedProfile(): FixtureProfile = profile

    fun bodyWeightDeliveries(): List<String> = bodyWeightDeliveries.toList()

    fun profileDeliveries(): List<Long> = profileDeliveries.toList()

    fun loseEditResponses() {
        workoutEdits = WorkoutEdits.COMMITTED_RESPONSE_LOST
    }

    fun deleteWorkout(id: Long) {
        workouts.removeAll { it.id == id }
    }

    fun editDeliveries(): List<Long> = editDeliveries.toList()

    fun loseExerciseResponses() {
        exerciseCreates = ExerciseCreates.COMMITTED_RESPONSE_LOST
    }

    fun putExercise(exercise: FixtureExercise) {
        exercises.removeAll { it.id == exercise.id }
        exercises += exercise
    }

    fun storedExercises(): List<FixtureExercise> = exercises.toList()

    fun exerciseDeliveries(): List<String> = exerciseDeliveries.toList()

    fun commitOrder(): List<String> = createOrder.toList()

    fun loseWorkoutResponses() {
        workoutCreates = WorkoutCreates.COMMITTED_RESPONSE_LOST
    }

    fun dropWorkoutRequests() {
        workoutCreates = WorkoutCreates.DROPPED
    }

    fun failWorkoutCreates() {
        workoutCreates = WorkoutCreates.SERVER_ERROR
    }

    fun refuseWorkoutCreates() {
        workoutCreates = WorkoutCreates.REFUSED
    }

    fun storedWorkouts(): List<FixtureWorkout> = workouts.toList()

    fun workoutDeliveries(): List<String> = deliveries.toList()

    fun holdWorkoutDetails(): CountDownLatch =
        CountDownLatch(1).also { releaseBlockedDetails = it }

    fun awaitHeldDetailRequest() {
        check(blockedDetails.await(20, TimeUnit.SECONDS)) { "The app never requested a Workout detail" }
    }

    override fun close() {
        releaseBlockedDetails?.countDown()
        commandGate?.countDown()
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
                awaitCommandGate()
                when {
                    loseTvResponse -> disconnected()
                    tvCommandCode != 200 -> error(tvCommandCode, "IR unavailable")
                    else -> ok(buildJsonObject { put("message", "Sent command") })
                }
            }
            path == "/api/ir/edifier" && request.method == "POST" -> {
                speakerRequests += Json.parseToJsonElement(checkNotNull(request.body).utf8()).jsonObject
                awaitCommandGate()
                when {
                    loseSpeakerResponse -> disconnected()
                    speakerCommandCode != 200 -> error(speakerCommandCode, "Speaker unavailable")
                    else -> ok(buildJsonObject { put("message", "Sent command") })
                }
            }
            path in listOf("/api/lights/on", "/api/lights/off") && request.method == "POST" -> {
                lightRequests += path to (request.body?.utf8() ?: "")
                awaitCommandGate()
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
                    awaitCommandGate()
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

    private fun exercisesJson() = JsonArray(exercises.map(::exerciseJson))

    internal fun profileJson() =
        buildJsonObject {
            put("height_cm", profile.heightCm)
            put("sex", profile.sex)
            put("hr_max_bpm", profile.hrMaxBpm)
            put("birth_date", profile.birthDate)
            profile.clientRevision?.let { put("client_revision", it) }
        }
}
