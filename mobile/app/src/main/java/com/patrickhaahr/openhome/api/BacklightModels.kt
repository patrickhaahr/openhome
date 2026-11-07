package com.patrickhaahr.openhome.api

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
sealed class BacklightSelection {
    @Serializable @SerialName("off") data object Off : BacklightSelection()
    @Serializable @SerialName("tv-sync") data class TvSync(val brightness: Int? = null) : BacklightSelection()
    @Serializable @SerialName("color") data class Color(val rgb: BacklightRgb, val brightness: Int) : BacklightSelection()
    @Serializable @SerialName("effect") data class Effect(val effect: String, val brightness: Int) : BacklightSelection()
}

@Serializable data class BacklightRgb(val red: Int, val green: Int, val blue: Int)
@Serializable data class BacklightEffect(val id: String, val name: String, val description: String)
@Serializable data class BacklightObservation(val power: Boolean, val brightness: Int, val mode: String, val observed_at: String)
@Serializable data class BacklightBedtime(val enabled: Boolean, val start: String, val end: String, val brightness: Int)

/** Independent server facts; an acknowledgement never substitutes for readback. */
@Serializable
data class BacklightStatus(
    val effects: List<BacklightEffect>,
    val setup: String,
    val reachability: String,
    val intended: BacklightSelection,
    val acknowledged: BacklightSelection?,
    val observed: BacklightObservation?,
    val stale: Boolean,
    val command: String,
    val recovery: String,
    val bedtime: BacklightBedtime,
    val control_source: String,
    val override_expires_at: String?,
    val next_transition: String?,
)

@Serializable
enum class BacklightProfileSource {
    @SerialName("capture") Capture,
    @SerialName("backup") Backup,
}

@Serializable data class BacklightProfileRequest(val source: BacklightProfileSource)

/** Omitting enabled preserves Axum's manual override on a time/brightness edit. */
@Serializable
data class BacklightBedtimeUpdate(
    val enabled: Boolean? = null,
    val start: String,
    val end: String,
    val brightness: Int,
)
