package com.patrickhaahr.openhome.e2e

import java.net.HttpURLConnection
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
import mockwebserver3.MockResponse
import mockwebserver3.SocketEffect

internal fun JsonObject.toFixtureWorkout(id: Long) =
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

internal fun JsonObject.text(key: String): String? = number(key)?.content

internal fun JsonObject.number(key: String): JsonPrimitive? =
    (this[key] as? JsonPrimitive)?.takeUnless { it is JsonNull }

internal fun disconnected(): MockResponse =
    MockResponse.Builder().onResponseStart(SocketEffect.CloseSocket()).build()

internal fun pageLimit(value: String?): Int = (value?.toIntOrNull() ?: 100).coerceIn(1, 500)

internal fun ok(json: Any): MockResponse =
    MockResponse.Builder()
        .code(HttpURLConnection.HTTP_OK)
        .addHeader("Content-Type", "application/json")
        .body(json.toString())
        .build()

internal fun error(status: Int, message: String): MockResponse =
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

internal fun exerciseJson(exercise: FixtureExercise) =
    buildJsonObject {
        put("id", exercise.id)
        put("name", exercise.name)
        put("category", exercise.category)
        put("muscle_group", exercise.muscleGroup)
        put("equipment", exercise.equipment)
    }

internal fun bodyWeightJson(entry: FixtureBodyWeight) =
    buildJsonObject {
        put("id", entry.id)
        put("date", entry.date)
        put("weight_kg", entry.weightKg)
    }

internal fun summaryJson(workout: FixtureWorkout) =
    buildJsonObject {
        put("id", workout.id)
        put("date", workout.date)
        put("name", workout.name)
    }

internal fun FakeAxum.detailJson(workout: FixtureWorkout): JsonObject =
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
