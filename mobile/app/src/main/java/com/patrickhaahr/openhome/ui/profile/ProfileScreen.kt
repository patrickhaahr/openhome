package com.patrickhaahr.openhome.ui.profile

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.BodyWeight
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.fitness.Profile
import com.patrickhaahr.openhome.fitness.ProfileRecords
import com.patrickhaahr.openhome.setup.SavedConnection
import com.patrickhaahr.openhome.setup.SetupService
import com.patrickhaahr.openhome.storage.SyncProblem
import com.patrickhaahr.openhome.sync.FitnessSyncStatus
import com.patrickhaahr.openhome.sync.SyncSnapshot
import com.patrickhaahr.openhome.sync.SyncStatus
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.Slab
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.formatCentimeters
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.formatKilograms
import com.patrickhaahr.openhome.ui.formatShortDay
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import com.patrickhaahr.openhome.ui.workout.SyncStatusLine
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

private const val CHART_POINTS = 60

class ProfileViewModel(
    fitness: FitnessRepository,
    setup: SetupService,
    private val records: ProfileRecords,
    private val syncStatus: FitnessSyncStatus,
    private val saveScope: CoroutineScope,
) : ViewModel() {
    val profile: StateFlow<Profile?> =
        fitness.observeProfile().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)
    val bodyWeight: StateFlow<List<BodyWeight>?> =
        fitness.observeBodyWeight().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)
    val connection: StateFlow<SavedConnection?> =
        flow { emit(setup.savedConnection()) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)
    val sync: StateFlow<SyncSnapshot?> =
        syncStatus.observe().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    fun syncNow() {
        saveScope.launch { syncStatus.syncNow() }
    }

    fun useOpenHomesBodyWeight(date: String) {
        saveScope.launch { records.useBodyWeightOnServer(date) }
    }
}

@Composable
fun ProfileRoute(onEdit: () -> Unit, onRecordBodyWeight: () -> Unit) {
    val viewModel =
        appViewModel { ProfileViewModel(it.fitness, it.setup, it.profileRecords, it.syncStatus, it.saveScope) }
    val profile by viewModel.profile.collectAsStateWithLifecycle()
    val bodyWeight by viewModel.bodyWeight.collectAsStateWithLifecycle()
    val connection by viewModel.connection.collectAsStateWithLifecycle()
    val sync by viewModel.sync.collectAsStateWithLifecycle()

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        item {
            ScreenHeader(title = "Profile", glyph = Glyph.Person) {
                sync?.let { SyncStatusLine(it.profile, onSyncNow = viewModel::syncNow) }
                GlyphKey(Glyph.Pencil, "Edit Profile", onEdit, tone = KeyTone.Raised, size = 48.dp)
            }
        }
        item { WeightHero(bodyWeight.orEmpty(), onRecordBodyWeight) }
        item {
            Column(Modifier.padding(horizontal = ScreenMargin), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    ProfileValue(Glyph.Ruler, "Height", profile?.heightCm?.let(::formatCentimeters), Modifier.weight(1f))
                    ProfileValue(Glyph.Heart, "Max Heart Rate", profile?.maxHeartRateBpm?.let { "$it bpm" }, Modifier.weight(1f))
                }
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    ProfileValue(
                        if (profile?.sex == "female") Glyph.Female else Glyph.Male,
                        "Sex",
                        profile?.sex?.replaceFirstChar(Char::uppercase),
                        Modifier.weight(1f),
                    )
                    ProfileValue(Glyph.Cake, "Birth date", profile?.birthDate?.let(::formatDay), Modifier.weight(1f))
                }
            }
        }
        item { ConnectionPlate(connection) }
        bodyWeight?.let { entries ->
            if (entries.isEmpty()) {
                item { Spoken("No Body Weight recorded yet.") }
            }
            items(entries, key = { it.date }) { entry ->
                Column(Modifier.padding(horizontal = ScreenMargin + 4.dp)) {
                    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(formatDay(entry.date), style = Type.Body, color = Lit.Muted, modifier = Modifier.weight(1f))
                        Text(formatKilograms(entry.weightKg), style = Type.Name, color = Lit.Ink)
                    }
                    BodyWeightSync(
                        status = sync?.bodyWeight(entry.date) ?: SyncStatus.Synced,
                        refusal = sync?.bodyWeightRefusal(entry.date),
                        onSyncNow = viewModel::syncNow,
                        onUseOpenHomes = { viewModel.useOpenHomesBodyWeight(entry.date) },
                    )
                }
            }
        }
    }
}

/** The figure a Profile is opened for: what you weigh, and which way it has been going. */
@Composable
private fun WeightHero(entries: List<BodyWeight>, onRecord: () -> Unit) {
    val latest = entries.maxByOrNull { it.date }
    Slab(Modifier.fillMaxWidth().padding(horizontal = ScreenMargin), padding = 20.dp) {
        Spoken("Body Weight", isHeading = true)
        Row(verticalAlignment = Alignment.Top) {
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(latest?.let { formatKilograms(it.weightKg).removeSuffix(" kg") } ?: "–", style = Type.Giant, color = Lit.Ink)
                    Text("kg", style = Type.Figure, color = Lit.Muted, modifier = Modifier.padding(bottom = 10.dp))
                }
                latest?.let { Text(formatShortDay(it.date), style = Type.Name, color = Lit.Muted) }
            }
            GlyphKey(Glyph.Plus, "Record Body Weight", onRecord, tone = KeyTone.Lamp, size = 64.dp)
        }
        if (entries.size > 1) {
            WeightLine(entries)
        } else {
            Spacer(Modifier.height(4.dp))
        }
    }
}

/** One lit line and nothing else. A point for each record, in order; no smoothing between them. */
@Composable
private fun WeightLine(entries: List<BodyWeight>) {
    val points = entries.sortedBy { it.date }.takeLast(CHART_POINTS).map { it.weightKg.toFloat() }
    val low = points.min()
    val high = points.max()
    val span = (high - low).takeIf { it > 0f } ?: 1f
    Canvas(Modifier.fillMaxWidth().height(96.dp).padding(vertical = 8.dp)) {
        val step = size.width / (points.size - 1)
        fun place(index: Int) = Offset(index * step, size.height - (points[index] - low) / span * size.height)
        val line =
            Path().apply {
                moveTo(place(0).x, place(0).y)
                for (index in 1..points.lastIndex) lineTo(place(index).x, place(index).y)
            }
        drawPath(line, Lit.Lamp, style = Stroke(2.5.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
        drawCircle(Lit.Lamp, radius = 5.dp.toPx(), center = place(points.lastIndex))
    }
}

@Composable
private fun ConnectionPlate(connection: SavedConnection?) {
    Spoken("Connection", isHeading = true)
    Slab(Modifier.fillMaxWidth().padding(horizontal = ScreenMargin), arrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            GlyphIcon(Glyph.Globe, tint = Lit.Muted, size = 20.dp)
            Text(connection?.baseUrl ?: "–", style = Type.Body, color = Lit.Ink, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            GlyphIcon(Glyph.Key, tint = Lit.Muted, size = 20.dp)
            GlyphIcon(
                if (connection?.apiKeyAvailable == false) Glyph.Warn else Glyph.Check,
                tint = if (connection?.apiKeyAvailable == false) Lit.Alarm else Lit.Lamp,
                size = 20.dp,
            )
        }
    }
    Spoken(connection?.let { if (it.apiKeyAvailable) "Saved on this device" else "Needs to be entered again" } ?: "Not set")
}

/** Quiet once it is synced, like the record it describes: only what is still on its way gets a mark. */
@Composable
private fun BodyWeightSync(status: SyncStatus, refusal: SyncProblem?, onSyncNow: () -> Unit, onUseOpenHomes: () -> Unit) {
    when {
        refusal == SyncProblem.DATE_TAKEN ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = "OpenHome already has a Body Weight for this date, so this one isn't synced.",
                    style = Type.Body,
                    color = Lit.Alarm,
                    modifier = Modifier.weight(1f),
                )
                GlyphKey(Glyph.Download, "Use OpenHome's", onUseOpenHomes, tone = KeyTone.Raised, size = 48.dp)
            }
        status != SyncStatus.Synced -> SyncStatusLine(status, onSyncNow = onSyncNow)
    }
}

/** A symbol over its value. A value not given is a dash, never a guess. */
@Composable
private fun ProfileValue(glyph: Glyph, label: String, value: String?, modifier: Modifier) {
    Slab(modifier.heightIn(min = 104.dp), arrangement = Arrangement.SpaceBetween) {
        GlyphIcon(glyph, tint = Lit.Lamp, size = 24.dp)
        Spoken(label)
        Text(value ?: "–", style = Type.Name, color = if (value == null) Lit.Faint else Lit.Ink, maxLines = 2)
        if (value == null) Spoken("Not set")
    }
}
