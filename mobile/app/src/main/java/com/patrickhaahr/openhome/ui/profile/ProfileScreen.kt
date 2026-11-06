package com.patrickhaahr.openhome.ui.profile

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
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
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.formatCentimeters
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.formatKilograms
import com.patrickhaahr.openhome.ui.workout.SyncStatusLine
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

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

    LazyColumn(modifier = Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item { ScreenHeader(title = "Profile") }
        item {
            Column(Modifier.padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Column {
                    ProfileValue("Height", profile?.heightCm?.let(::formatCentimeters))
                    ProfileValue("Sex", profile?.sex?.replaceFirstChar(Char::uppercase))
                    ProfileValue("Birth date", profile?.birthDate?.let(::formatDay))
                    ProfileValue("Max Heart Rate", profile?.maxHeartRateBpm?.let { "$it bpm" })
                }
                sync?.let { SyncStatusLine(it.profile, onSyncNow = viewModel::syncNow) }
                OutlinedButton(onClick = onEdit, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) {
                    Text("Edit Profile")
                }
            }
        }
        item {
            SectionTitle("Connection")
            Column(Modifier.padding(horizontal = 20.dp)) {
                ProfileValue("Base URL", connection?.baseUrl)
                ProfileValue(
                    "API Key",
                    connection?.let { if (it.apiKeyAvailable) "Saved on this device" else "Needs to be entered again" },
                )
            }
        }
        item {
            SectionTitle("Body Weight")
            Button(
                onClick = onRecordBodyWeight,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp).heightIn(min = 56.dp),
            ) {
                Text("Record Body Weight")
            }
        }
        bodyWeight?.let { entries ->
            if (entries.isEmpty()) {
                item {
                    Text(
                        text = "No Body Weight recorded yet.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 12.dp),
                    )
                }
            }
            items(entries, key = { it.date }) { entry ->
                Column(Modifier.padding(horizontal = 20.dp)) {
                    ProfileValue(formatDay(entry.date), formatKilograms(entry.weightKg)) {
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
}

/** Quiet once it is synced, like the record it describes: only what is still on its way gets a line. */
@Composable
private fun BodyWeightSync(status: SyncStatus, refusal: SyncProblem?, onSyncNow: () -> Unit, onUseOpenHomes: () -> Unit) {
    when {
        refusal == SyncProblem.DATE_TAKEN ->
            Column {
                Text(
                    text = "OpenHome already has a Body Weight for this date, so this one isn't synced.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                )
                TextButton(onClick = onUseOpenHomes) { Text("Use OpenHome's") }
            }
        status != SyncStatus.Synced -> SyncStatusLine(status, onSyncNow = onSyncNow)
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.headlineSmall,
        modifier =
            Modifier
                .padding(start = 20.dp, end = 20.dp, top = 28.dp, bottom = 8.dp)
                .semantics { heading() },
    )
}

@Composable
private fun ProfileValue(label: String, value: String?, below: @Composable () -> Unit = {}) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = 52.dp)
                .semantics(mergeDescendants = true) {},
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text = label, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
        Text(
            text = value ?: "Not set",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.End,
            modifier = Modifier.weight(1f),
        )
    }
    below()
    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
}
