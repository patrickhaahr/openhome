package com.patrickhaahr.openhome.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import com.patrickhaahr.openhome.fitness.BodyWeightSave
import com.patrickhaahr.openhome.fitness.EARLIEST_BIRTH_DATE
import com.patrickhaahr.openhome.fitness.PROFILE_SEXES
import com.patrickhaahr.openhome.fitness.Profile
import com.patrickhaahr.openhome.fitness.ProfileDraft
import com.patrickhaahr.openhome.fitness.ProfileRecords
import com.patrickhaahr.openhome.fitness.ProfileSave
import com.patrickhaahr.openhome.fitness.latestBirthDate
import com.patrickhaahr.openhome.ui.DayPicker
import com.patrickhaahr.openhome.ui.FieldShape
import com.patrickhaahr.openhome.ui.FormField
import com.patrickhaahr.openhome.ui.ProblemText
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.SlabShape
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.appViewModel
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.SlabKey
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.formatDay
import com.patrickhaahr.openhome.ui.formatKilograms
import com.patrickhaahr.openhome.ui.plainNumber
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type
import java.time.LocalDate
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

private const val SAVED_FIRST = "Saved on this device first, then synced over Wi-Fi."
private const val SAVE_FAILED = "Couldn't save on this device. Please try again."

private data class ProfileProblems(
    val height: String? = null,
    val maxHeartRate: String? = null,
    val birthDate: String? = null,
    val general: String? = null,
)

private data class BodyWeightProblems(val weight: String? = null, val date: String? = null, val general: String? = null)

/** Saves in the app's scope: once the user has pressed Save, leaving the screen cannot lose it. */
private class ProfileFormsViewModel(
    private val records: ProfileRecords,
    private val saveScope: CoroutineScope,
) : ViewModel() {
    private val loadedNow = MutableStateFlow<Profile?>(null)
    private val profileProblemsNow = MutableStateFlow(ProfileProblems())
    private val bodyWeightProblemsNow = MutableStateFlow(BodyWeightProblems())
    private var saving = false

    val loaded: StateFlow<Profile?> = loadedNow
    val profileProblems: StateFlow<ProfileProblems> = profileProblemsNow
    val bodyWeightProblems: StateFlow<BodyWeightProblems> = bodyWeightProblemsNow

    fun loadProfile() {
        viewModelScope.launch { loadedNow.value = records.loadProfile() ?: Profile(null, null, null, null) }
    }

    fun saveProfile(draft: ProfileDraft, onSaved: () -> Unit) {
        if (saving) return
        saving = true
        saveScope.launch {
            // Stays set once saved: the form is on its way out and a second tap must not save again.
            when (val outcome = runCatching { records.saveProfile(draft) }.getOrNull()) {
                ProfileSave.Saved -> return@launch onSaved()
                is ProfileSave.Invalid ->
                    profileProblemsNow.value =
                        outcome.problems.let { ProfileProblems(it.height, it.maxHeartRate, it.birthDate) }
                null -> profileProblemsNow.value = ProfileProblems(general = SAVE_FAILED)
            }
            saving = false
        }
    }

    /** What was said about the date before no longer applies to the one now chosen. */
    fun bodyWeightDateChanged() {
        bodyWeightProblemsNow.value = bodyWeightProblemsNow.value.copy(date = null)
    }

    fun recordBodyWeight(date: LocalDate, weightKg: String, onSaved: () -> Unit) {
        if (saving) return
        saving = true
        saveScope.launch {
            when (val outcome = runCatching { records.recordBodyWeight(date, weightKg) }.getOrNull()) {
                BodyWeightSave.Saved -> return@launch onSaved()
                BodyWeightSave.InvalidWeight ->
                    bodyWeightProblemsNow.value = BodyWeightProblems(weight = "Enter a weight in kg above 0")
                is BodyWeightSave.AlreadyRecorded ->
                    bodyWeightProblemsNow.value =
                        BodyWeightProblems(
                            date =
                                "${formatDay(outcome.existing.date)} is already recorded as " +
                                    "${formatKilograms(outcome.existing.weightKg)}. Each date is recorded once, so " +
                                    "that record stays as it is. Choose another date.",
                        )
                null -> bodyWeightProblemsNow.value = BodyWeightProblems(general = SAVE_FAILED)
            }
            saving = false
        }
    }
}

@Composable
private fun profileFormsViewModel(): ProfileFormsViewModel =
    appViewModel { ProfileFormsViewModel(it.profileRecords, it.saveScope) }

/** Works without a connection: the edit is the Profile as soon as it is saved on the device. */
@Composable
fun EditProfileRoute(onSaved: () -> Unit) {
    val viewModel = profileFormsViewModel()
    val loaded by viewModel.loaded.collectAsStateWithLifecycle()
    val problems by viewModel.profileProblems.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { viewModel.loadProfile() }
    loaded?.let { profile ->
        ProfileForm(initial = profile, problems = problems, onSave = { viewModel.saveProfile(it, onSaved) })
    }
}

@Composable
private fun ProfileForm(initial: Profile, problems: ProfileProblems, onSave: (ProfileDraft) -> Unit) {
    var height by rememberSaveable { mutableStateOf(initial.heightCm?.let(::plainNumber).orEmpty()) }
    var sex by rememberSaveable { mutableStateOf(initial.sex) }
    var birthDate by rememberSaveable { mutableStateOf(initial.birthDate) }
    var maxHeartRate by rememberSaveable { mutableStateOf(initial.maxHeartRateBpm?.toString().orEmpty()) }
    var choosingBirthDate by rememberSaveable { mutableStateOf(false) }

    Column(Modifier.fillMaxSize().imePadding().verticalScroll(rememberScrollState())) {
        ScreenHeader(title = "Edit Profile", glyph = Glyph.Pencil)
        Column(
            modifier = Modifier.fillMaxWidth().padding(horizontal = ScreenMargin),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            FormField(
                value = height,
                onValueChange = { height = it },
                label = "Height in cm (optional)",
                glyph = Glyph.Ruler,
                unit = "cm",
                problem = problems.height,
                keyboardType = KeyboardType.Decimal,
            )
            SexField(selected = sex, stored = initial.sex, onSelect = { sex = it })
            BirthDateField(
                date = birthDate,
                problem = problems.birthDate,
                onChoose = { choosingBirthDate = true },
                onClear = { birthDate = null },
            )
            FormField(
                value = maxHeartRate,
                onValueChange = { maxHeartRate = it },
                label = "Max Heart Rate in bpm (optional)",
                glyph = Glyph.Heart,
                unit = "bpm",
                problem = problems.maxHeartRate,
                keyboardType = KeyboardType.Number,
            )
            problems.general?.let { ProblemText(it) }
            SlabKey(
                Glyph.Check,
                "Save Profile",
                { onSave(ProfileDraft(height, sex, birthDate, maxHeartRate)) },
                modifier = Modifier.fillMaxWidth(),
            )
            Spoken(SAVED_FIRST)
        }
    }
    if (choosingBirthDate) {
        DayPicker(
            date = birthDate,
            earliest = EARLIEST_BIRTH_DATE,
            latest = latestBirthDate(),
            onChosen = { birthDate = it.toString() },
            onDismiss = { choosingBirthDate = false },
        )
    }
}

/** A sex stored by another client that nutrition cannot use stays selectable until it is changed. Tapping the chosen one clears it. */
@Composable
private fun SexField(selected: String?, stored: String?, onSelect: (String?) -> Unit) {
    val choices = (PROFILE_SEXES + listOfNotNull(stored)).distinct()
    Spoken("Sex (optional)")
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        choices.forEach { sex ->
            val label = sex.replaceFirstChar(Char::uppercase)
            val tone = if (selected == sex) KeyTone.Lamp else KeyTone.Slab
            val toggle = { onSelect(sex.takeUnless { it == selected }) }
            when (sex) {
                "male", "female" ->
                    GlyphKey(
                        if (sex == "male") Glyph.Male else Glyph.Female,
                        label,
                        toggle,
                        modifier = Modifier.weight(1f),
                        tone = tone,
                        size = 88.dp,
                        glyphSize = 36.dp,
                        shape = SlabShape,
                    )
                else ->
                    Column(
                        Modifier
                            .weight(1f)
                            .height(88.dp)
                            .key(label, toggle, shape = SlabShape)
                            .background(tone.fill, SlabShape)
                            .padding(16.dp),
                    ) { Text(label, style = Type.Name, color = tone.ink) }
            }
        }
    }
}

@Composable
private fun BirthDateField(date: String?, problem: String?, onChoose: () -> Unit, onClear: () -> Unit) {
    Spoken("Birth date (optional)")
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DateKey(date, date?.let(::formatDay) ?: "Choose birth date", Glyph.Cake, onChoose, Modifier.weight(1f))
        if (date != null) GlyphKey(Glyph.Close, "Clear birth date", onClear, tone = KeyTone.Quiet)
    }
    problem?.let { ProblemText(it) }
}

@Composable
private fun DateKey(date: String?, description: String, glyph: Glyph, onClick: () -> Unit, modifier: Modifier = Modifier) {
    Row(
        modifier
            .height(72.dp)
            .key(description, onClick, shape = FieldShape, depth = 0.98f)
            .background(Lit.Slab, FieldShape)
            .padding(horizontal = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        GlyphIcon(glyph, tint = Lit.Lamp, size = 26.dp)
        Text(date?.let(::formatDay) ?: "–", style = Type.Name, color = if (date == null) Lit.Faint else Lit.Ink)
    }
}

/** Recording only: a date already recorded stays as it is, here and on the server. */
@Composable
fun RecordBodyWeightRoute(onSaved: () -> Unit) {
    val viewModel = profileFormsViewModel()
    val problems by viewModel.bodyWeightProblems.collectAsStateWithLifecycle()
    var date by rememberSaveable { mutableStateOf(LocalDate.now().toString()) }
    var weight by rememberSaveable { mutableStateOf("") }
    var choosingDate by rememberSaveable { mutableStateOf(false) }

    Column(Modifier.fillMaxSize().imePadding().verticalScroll(rememberScrollState())) {
        ScreenHeader(title = "Record Body Weight", glyph = Glyph.Scale)
        Column(
            modifier = Modifier.fillMaxWidth().padding(horizontal = ScreenMargin),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Spoken("Date")
            DateKey(date, formatDay(date), Glyph.Calendar, { choosingDate = true }, Modifier.fillMaxWidth())
            problems.date?.let { ProblemText(it) }
            FormField(
                value = weight,
                onValueChange = { weight = it },
                label = "Weight in kg",
                glyph = Glyph.Scale,
                unit = "kg",
                problem = problems.weight,
                keyboardType = KeyboardType.Decimal,
            )
            problems.general?.let { ProblemText(it) }
            SlabKey(
                Glyph.Check,
                "Save Body Weight",
                { viewModel.recordBodyWeight(LocalDate.parse(date), weight, onSaved) },
                modifier = Modifier.fillMaxWidth(),
            )
            Spoken(SAVED_FIRST)
        }
    }
    if (choosingDate) {
        DayPicker(
            date = date,
            onChosen = {
                date = it.toString()
                viewModel.bodyWeightDateChanged()
            },
            onDismiss = { choosingDate = false },
        )
    }
}
