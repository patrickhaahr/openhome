package com.patrickhaahr.openhome.ui.workout

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.fitness.EXERCISE_CATEGORIES
import com.patrickhaahr.openhome.fitness.Exercise
import com.patrickhaahr.openhome.fitness.ExerciseFilter
import com.patrickhaahr.openhome.fitness.muscleGroups
import com.patrickhaahr.openhome.storage.SyncProblem
import com.patrickhaahr.openhome.ui.EmptyState
import com.patrickhaahr.openhome.ui.ScreenHeader

/** What storage has answered so far. A failed read says so rather than looking like an empty Library. */
sealed interface LibraryLookup<out T> {
    data object Loading : LibraryLookup<Nothing>

    data class Loaded<T>(val rows: List<T>) : LibraryLookup<T>

    data object Unreadable : LibraryLookup<Nothing>
}

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
    lookup: LibraryLookup<T>,
    exerciseOf: (T) -> Exercise,
    onCreate: ((String) -> Unit)?,
    row: @Composable (T) -> Unit,
) {
    var query by rememberSaveable { mutableStateOf("") }
    var category by rememberSaveable { mutableStateOf<String?>(null) }
    var muscleGroup by rememberSaveable { mutableStateOf<String?>(null) }
    val filter = ExerciseFilter(query, category, muscleGroup)

    Column(Modifier.fillMaxSize().imePadding()) {
        ScreenHeader(title = title)
        OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            label = { Text("Search Exercises") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
        )
        val all = (lookup as? LibraryLookup.Loaded)?.rows.orEmpty().map(exerciseOf)
        FilterChips(
            options = EXERCISE_CATEGORIES,
            selected = category,
            label = ::categoryLabel,
            onSelect = { category = it },
        )
        FilterChips(
            options = all.muscleGroups(),
            selected = muscleGroup,
            label = { it },
            onSelect = { muscleGroup = it },
        )
        Box(Modifier.weight(1f)) {
            when (lookup) {
                LibraryLookup.Loading -> Unit
                LibraryLookup.Unreadable ->
                    EmptyState(
                        title = "Can't read your Library",
                        message = "Nothing was lost. Close and reopen the app to try again.",
                    )
                is LibraryLookup.Loaded -> {
                    val matches = lookup.rows.filter { filter.matches(exerciseOf(it)) }
                    when {
                        lookup.rows.isEmpty() ->
                            EmptyState(
                                title = "No Exercises yet",
                                message = if (onCreate != null) "Create one below, even without a connection." else "Create one in the Library, even without a connection.",
                            )
                        matches.isEmpty() ->
                            EmptyState(
                                title = "No matching Exercises",
                                message = if (onCreate != null) "Try a different search, or create it below." else "Try a different search, or create it in the Library.",
                            ) {
                                TextButton(
                                    onClick = {
                                        query = ""
                                        category = null
                                        muscleGroup = null
                                    },
                                ) {
                                    Text("Clear search and filters")
                                }
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
        if (onCreate != null) {
            OutlinedButton(
                onClick = { onCreate(query.trim()) },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp).heightIn(min = 56.dp),
            ) {
                Text("Create Exercise")
            }
        }
    }
}

@Composable
private fun <T> ExerciseList(
    matches: List<T>,
    countLabel: String?,
    exerciseOf: (T) -> Exercise,
    row: @Composable (T) -> Unit,
) {
    LazyColumn(contentPadding = PaddingValues(vertical = 8.dp)) {
        if (countLabel != null) {
            item(key = "count") {
                Text(
                    text = countLabel,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp),
                )
            }
        }
        items(matches, key = { exerciseOf(it).id }) { match ->
            row(match)
            HorizontalDivider(
                color = MaterialTheme.colorScheme.outlineVariant,
                modifier = Modifier.padding(horizontal = 20.dp),
            )
        }
    }
}

/** Tapping the chosen one clears it. A horizontal row, so a long list never pushes the results away. */
@Composable
private fun FilterChips(
    options: List<String>,
    selected: String?,
    label: (String) -> String,
    onSelect: (String?) -> Unit,
) {
    if (options.isEmpty()) return
    Row(
        modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        options.forEach { option ->
            val isSelected = option.equals(selected, ignoreCase = true)
            FilterChip(
                selected = isSelected,
                onClick = { onSelect(if (isSelected) null else option) },
                label = { Text(label(option)) },
            )
        }
    }
}

/** One Exercise with its optional metadata. Only rows with something to do are tappable. */
@Composable
fun ExerciseRow(
    exercise: Exercise,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    onClickLabel: String? = null,
    below: @Composable () -> Unit = {},
) {
    val tappable =
        if (onClick != null) Modifier.clickable(role = Role.Button, onClickLabel = onClickLabel, onClick = onClick) else Modifier
    Column(
        modifier =
            modifier
                .fillMaxWidth()
                .then(tappable)
                .heightIn(min = 64.dp)
                .padding(horizontal = 20.dp, vertical = 10.dp),
    ) {
        Text(text = exercise.name, style = MaterialTheme.typography.titleMedium)
        Text(
            text = exerciseDetails(exercise),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        below()
    }
}

