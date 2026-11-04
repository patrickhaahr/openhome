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
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
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
import com.patrickhaahr.openhome.setup.SavedConnection
import com.patrickhaahr.openhome.setup.SetupService
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.formatCentimeters
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.formatKilograms
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.stateIn

class ProfileViewModel(fitness: FitnessRepository, setup: SetupService) : ViewModel() {
    val profile: StateFlow<Profile?> =
        fitness.observeProfile().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)
    val bodyWeight: StateFlow<List<BodyWeight>?> =
        fitness.observeBodyWeight().stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)
    val connection: StateFlow<SavedConnection?> =
        flow { emit(setup.savedConnection()) }
            .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)
}

@Composable
fun ProfileRoute() {
    val viewModel = appViewModel { ProfileViewModel(it.fitness, it.setup) }
    val profile by viewModel.profile.collectAsStateWithLifecycle()
    val bodyWeight by viewModel.bodyWeight.collectAsStateWithLifecycle()
    val connection by viewModel.connection.collectAsStateWithLifecycle()

    LazyColumn(modifier = Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item { ScreenHeader(title = "Profile") }
        item {
            Column(Modifier.padding(horizontal = 20.dp)) {
                ProfileValue("Height", profile?.heightCm?.let(::formatCentimeters))
                ProfileValue("Sex", profile?.sex?.replaceFirstChar(Char::uppercase))
                ProfileValue("Birth date", profile?.birthDate?.let(::formatDay))
                ProfileValue("Max Heart Rate", profile?.maxHeartRateBpm?.let { "$it bpm" })
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
        item { SectionTitle("Body Weight") }
        bodyWeight?.let { entries ->
            if (entries.isEmpty()) {
                item {
                    Text(
                        text = "No Body Weight recorded yet.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 20.dp),
                    )
                }
            }
            items(entries, key = { it.date }) { entry ->
                Column(Modifier.padding(horizontal = 20.dp)) {
                    ProfileValue(formatDay(entry.date), formatKilograms(entry.weightKg))
                }
            }
        }
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
private fun ProfileValue(label: String, value: String?) {
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
    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
}
