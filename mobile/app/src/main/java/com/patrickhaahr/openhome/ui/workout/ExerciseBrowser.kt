package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.fitness.EXERCISE_CATEGORIES
import com.patrickhaahr.openhome.fitness.Exercise
import com.patrickhaahr.openhome.fitness.ExerciseFilter
import com.patrickhaahr.openhome.fitness.muscleGroups
import com.patrickhaahr.openhome.storage.SyncProblem
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.FormField
import com.patrickhaahr.openhome.ui.ScreenHeader
import com.patrickhaahr.openhome.ui.ScreenMargin
import com.patrickhaahr.openhome.ui.SlabShape
import com.patrickhaahr.openhome.ui.Spoken
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphIcon
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.design.key
import com.patrickhaahr.openhome.ui.theme.Lit
import com.patrickhaahr.openhome.ui.theme.Type

/** What storage has answered so far. A failed read says so rather than looking like an empty Library. */
sealed interface LibraryLookup<out T> {
    data object Loading : LibraryLookup<Nothing>

    data class Loaded<T>(val rows: List<T>) : LibraryLookup<T>

    data object Unreadable : LibraryLookup<Nothing>
}

fun categoryGlyph(category: String): Glyph = if (category.equals("gym", ignoreCase = true)) Glyph.Barbell else Glyph.Rings

fun categoryLabel(category: String): String = category.replaceFirstChar { it.uppercase() }

fun exerciseDetails(exercise: Exercise): String =
    listOfNotNull(exercise.category, exercise.muscleGroup, exercise.equipment).joinToString(separator = " · ")

/** What to do about a refused Exercise; nothing logged with it is at risk meanwhile. */
fun exerciseRefusalMessage(problem: SyncProblem): String =
    when (problem) {
        SyncProblem.NAME_TAKEN -> "OpenHome already has an Exercise with this name. Tap to rename or use it."
        else -> "OpenHome refused this Exercise. Tap to check its details."
    }

/** Search, filters, the list and, given [onCreate], a way to create what is missing: the same for Library and picker. */
@Composable
fun <T> ExerciseBrowser(
    title: String,
    glyph: Glyph,
    lookup: LibraryLookup<T>,
    exerciseOf: (T) -> Exercise,
    onCreate: ((String) -> Unit)?,
    row: @Composable (T) -> Unit,
) {
    var query by rememberSaveable { mutableStateOf("") }
    var category by rememberSaveable { mutableStateOf<String?>(null) }
    var muscleGroup by rememberSaveable { mutableStateOf<String?>(null) }
    val filter = ExerciseFilter(query, category, muscleGroup)
    val all = (lookup as? LibraryLookup.Loaded)?.rows.orEmpty().map(exerciseOf)

    Box(Modifier.fillMaxSize().imePadding()) {
        Column(Modifier.fillMaxSize()) {
            ScreenHeader(title = title, glyph = glyph, figure = all.size.takeIf { it > 0 }?.toString())
            FormField(
                value = query,
                onValueChange = { query = it },
                label = "Search Exercises",
                glyph = Glyph.Search,
                modifier = Modifier.fillMaxWidth().padding(horizontal = ScreenMargin),
            )
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .padding(horizontal = ScreenMargin, vertical = 10.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                EXERCISE_CATEGORIES.forEach { option ->
                    val isSelected = option.equals(category, ignoreCase = true)
                    GlyphKey(
                        categoryGlyph(option),
                        categoryLabel(option),
                        { category = if (isSelected) null else option },
                        tone = if (isSelected) KeyTone.Lamp else KeyTone.Raised,
                        size = 48.dp,
                    )
                }
                all.muscleGroups().forEach { option ->
                    val isSelected = option.equals(muscleGroup, ignoreCase = true)
                    TextChip(option, isSelected) { muscleGroup = if (isSelected) null else option }
                }
            }
            Box(Modifier.weight(1f)) {
                when (lookup) {
                    LibraryLookup.Loading -> Unit
                    LibraryLookup.Unreadable ->
                        EmptyState(
                            title = "Can't read your Library",
                            message = "Nothing was lost. Close and reopen the app to try again.",
                            glyph = Glyph.Warn,
                            readable = true,
                        )
                    is LibraryLookup.Loaded -> {
                        val matches = lookup.rows.filter { filter.matches(exerciseOf(it)) }
                        when {
                            lookup.rows.isEmpty() ->
                                EmptyState(
                                    title = "No Exercises yet",
                                    message = if (onCreate != null) "Create one below, even without a connection." else "Create one in the Library, even without a connection.",
                                    glyph = Glyph.Stack,
                                )
                            matches.isEmpty() ->
                                EmptyState(
                                    title = "No matching Exercises",
                                    message = if (onCreate != null) "Try a different search, or create it below." else "Try a different search, or create it in the Library.",
                                    glyph = Glyph.Search,
                                ) {
                                    GlyphKey(
                                        Glyph.Close,
                                        "Clear search and filters",
                                        {
                                            query = ""
                                            category = null
                                            muscleGroup = null
                                        },
                                    )
                                }
                            else ->
                                ExerciseList(
                                    matches = matches,
                                    countLabel = if (filter.isActive) "${matches.size} of ${lookup.rows.size}" else null,
                                    exerciseOf = exerciseOf,
                                    row = row,
                                )
                        }
                    }
                }
            }
        }
        if (onCreate != null) {
            GlyphKey(
                Glyph.Plus,
                "Create Exercise",
                { onCreate(query.trim()) },
                modifier = Modifier.align(Alignment.BottomEnd).padding(ScreenMargin),
                tone = KeyTone.Lamp,
                size = 72.dp,
            )
        }
    }
}

@Composable
private fun TextChip(text: String, selected: Boolean, onClick: () -> Unit) {
    Box(
        Modifier
            .height(48.dp)
            .key(text, onClick)
            .background(if (selected) Lit.Lamp else Lit.Raised, CircleShape)
            .padding(horizontal = 18.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = Type.Name, color = if (selected) Lit.OnLamp else Lit.Ink, maxLines = 1)
    }
}

@Composable
private fun <T> ExerciseList(
    matches: List<T>,
    countLabel: String?,
    exerciseOf: (T) -> Exercise,
    row: @Composable (T) -> Unit,
) {
    LazyColumn(contentPadding = PaddingValues(start = ScreenMargin, end = ScreenMargin, top = 4.dp, bottom = 112.dp)) {
        if (countLabel != null) {
            item(key = "count") {
                Text(
                    text = countLabel,
                    style = Type.Small,
                    color = Lit.Muted,
                    modifier = Modifier.padding(horizontal = 4.dp, vertical = 4.dp),
                )
            }
        }
        items(matches, key = { exerciseOf(it).id }) { match -> row(match) }
    }
}

/** One Exercise, marked by what kind it is. Only rows with something to do are tappable. */
@Composable
fun ExerciseRow(
    exercise: Exercise,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    onClickLabel: String? = null,
    below: @Composable () -> Unit = {},
) {
    val tappable =
        if (onClick != null) Modifier.key(onClickLabel.orEmpty(), onClick, shape = SlabShape, depth = 0.98f) else Modifier
    Row(
        modifier =
            modifier
                .fillMaxWidth()
                .then(tappable)
                .heightIn(min = 68.dp)
                .padding(horizontal = 4.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        GlyphIcon(categoryGlyph(exercise.category), tint = Lit.Muted, size = 26.dp)
        Column(Modifier.weight(1f)) {
            Text(text = exercise.name, style = Type.Name, color = Lit.Ink)
            val details = listOfNotNull(exercise.muscleGroup, exercise.equipment).joinToString(separator = " · ")
            if (details.isNotEmpty()) {
                Text(text = details, style = Type.Body, color = Lit.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            Spoken(exerciseDetails(exercise))
            below()
        }
    }
}
