package com.patrickhaahr.openhome.e2e

import java.net.HttpURLConnection
import java.util.concurrent.TimeUnit
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.double
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import mockwebserver3.MockResponse
import mockwebserver3.RecordedRequest

internal fun FakeAxum.createWorkout(request: RecordedRequest): MockResponse {
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

internal fun FakeAxum.editWorkout(id: Long, request: RecordedRequest): MockResponse {
    val body = Json.parseToJsonElement(checkNotNull(request.body).utf8()).jsonObject
    val revision = checkNotNull(body["client_revision"]).jsonPrimitive.long
    editDeliveries += revision
    val answer = commitEdit(id, revision, body)
    return if (workoutEdits == WorkoutEdits.COMMITTED_RESPONSE_LOST) disconnected() else answer
}

private fun FakeAxum.commitEdit(id: Long, revision: Long, body: JsonObject): MockResponse =
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

internal fun FakeAxum.createBodyWeight(request: RecordedRequest): MockResponse {
    val body = Json.parseToJsonElement(checkNotNull(request.body).utf8()).jsonObject
    val requestId = checkNotNull(body["request_id"]).jsonPrimitive.content
    bodyWeightDeliveries += requestId
    val answer = commitBodyWeight(requestId, body)
    return if (losingBodyWeightResponses) disconnected() else answer
}

private fun FakeAxum.commitBodyWeight(requestId: String, body: JsonObject): MockResponse =
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

internal fun FakeAxum.editProfile(request: RecordedRequest): MockResponse {
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

internal fun FakeAxum.createExercise(request: RecordedRequest): MockResponse {
    val body = Json.parseToJsonElement(checkNotNull(request.body).utf8()).jsonObject
    val requestId = checkNotNull(body["request_id"]).jsonPrimitive.content
    exerciseDeliveries += requestId
    val answer = commitExercise(requestId, body)
    return if (exerciseCreates == ExerciseCreates.COMMITTED_RESPONSE_LOST) disconnected() else answer
}

private fun FakeAxum.commitExercise(requestId: String, body: JsonObject): MockResponse =
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

private fun FakeAxum.commitWorkout(requestId: String, body: JsonObject): MockResponse =
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

internal fun FakeAxum.workoutDetail(id: Long): MockResponse {
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

private const val UNPROCESSABLE_ENTITY = 422

internal enum class WorkoutEdits {
    ANSWERED,
    COMMITTED_RESPONSE_LOST,
}

internal enum class ExerciseCreates {
    ANSWERED,
    COMMITTED_RESPONSE_LOST,
}

internal enum class WorkoutCreates {
    ANSWERED,
    COMMITTED_RESPONSE_LOST,
    DROPPED,
    SERVER_ERROR,
    REFUSED,
}
