package com.patrickhaahr.openhome.ui

import java.math.BigDecimal
import java.text.DecimalFormat
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.DateTimeParseException

private val DAY_FORMAT = DateTimeFormatter.ofPattern("EEE, d MMM yyyy")
private val SHORT_DAY_FORMAT = DateTimeFormatter.ofPattern("d MMM")
private val MONTH_FORMAT = DateTimeFormatter.ofPattern("MMM")
private val WEEKDAY_FORMAT = DateTimeFormatter.ofPattern("EEE")
private const val DAY_DIGITS = 2

/** Renders a calendar day as written; a day has no time zone to convert through. */
fun formatDay(isoDay: String): String = formatted(isoDay, DAY_FORMAT)

fun formatShortDay(isoDay: String): String = formatted(isoDay, SHORT_DAY_FORMAT)

fun formatMonth(isoDay: String): String = formatted(isoDay, MONTH_FORMAT)

fun formatWeekday(isoDay: String): String = formatted(isoDay, WEEKDAY_FORMAT)

fun dayOfMonth(isoDay: String): String =
    runCatching { LocalDate.parse(isoDay).dayOfMonth.toString().padStart(DAY_DIGITS, '0') }.getOrDefault(isoDay)

fun yearOf(isoDay: String): Int? = runCatching { LocalDate.parse(isoDay).year }.getOrNull()

private fun formatted(isoDay: String, format: DateTimeFormatter): String =
    try {
        LocalDate.parse(isoDay).format(format)
    } catch (_: DateTimeParseException) {
        isoDay
    }

fun formatKilograms(kilograms: Double): String = "${DecimalFormat("0.##").format(kilograms)} kg"

fun formatCentimeters(centimeters: Double): String = "${DecimalFormat("0.##").format(centimeters)} cm"

/** The number as typed into a field: every digit it has, and none it doesn't. */
fun plainNumber(value: Double): String = BigDecimal(value.toString()).stripTrailingZeros().toPlainString()

fun pluralize(count: Int, singular: String, plural: String = "${singular}s"): String =
    "$count ${if (count == 1) singular else plural}"
