package com.patrickhaahr.openhome.ui

import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDefaults
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SelectableDates
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.unit.dp
import com.patrickhaahr.openhome.ui.design.Glyph
import com.patrickhaahr.openhome.ui.design.GlyphKey
import com.patrickhaahr.openhome.ui.design.KeyTone
import com.patrickhaahr.openhome.ui.theme.Lit
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset

/** Chooses a calendar day, starting on [date] when there is one, from [earliest] up to [latest]. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DayPicker(
    date: String?,
    onChosen: (LocalDate) -> Unit,
    onDismiss: () -> Unit,
    earliest: LocalDate = LocalDate.MIN,
    latest: LocalDate? = null,
) {
    val lastDay = remember(latest) { latest ?: LocalDate.now() }
    val state =
        rememberDatePickerState(
            initialSelectedDateMillis = date?.let { LocalDate.parse(it).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli() },
            selectableDates =
                object : SelectableDates {
                    override fun isSelectableDate(utcTimeMillis: Long) = utcTimeMillis.toUtcDay() in earliest..lastDay

                    override fun isSelectableYear(year: Int) = year in earliest.year..lastDay.year
                },
        )
    DatePickerDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            GlyphKey(
                Glyph.Check,
                "Set date",
                {
                    state.selectedDateMillis?.let { onChosen(it.toUtcDay()) }
                    onDismiss()
                },
                tone = KeyTone.Lamp,
                size = 52.dp,
            )
        },
        dismissButton = { GlyphKey(Glyph.Close, "Cancel", onDismiss, tone = KeyTone.Quiet, size = 52.dp) },
        colors = DatePickerDefaults.colors(containerColor = Lit.Slab),
    ) {
        DatePicker(state = state, colors = DatePickerDefaults.colors(containerColor = Lit.Slab), title = null, headline = null, showModeToggle = false)
    }
}

private fun Long.toUtcDay(): LocalDate = Instant.ofEpochMilli(this).atZone(ZoneOffset.UTC).toLocalDate()
