package com.patrickhaahr.openhome.ui

import java.math.BigDecimal
import java.text.DecimalFormat
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.DateTimeParseException

private val DAY_FORMAT = DateTimeFormatter.ofPattern("EEE, d MMM yyyy")

/** Renders a calendar day as written; a day has no time zone to convert through. */
fun formatDay(isoDay: String): String =
    try {
        LocalDate.parse(isoDay).format(DAY_FORMAT)
    } catch (_: DateTimeParseException) {
        isoDay
    }

fun formatKilograms(kilograms: Double): String = "${DecimalFormat("0.##").format(kilograms)} kg"

fun formatCentimeters(centimeters: Double): String = "${DecimalFormat("0.##").format(centimeters)} cm"

/** The number as typed into a field: every digit it has, and none it doesn't. */
fun plainNumber(value: Double): String = BigDecimal(value.toString()).stripTrailingZeros().toPlainString()

fun pluralize(count: Int, singular: String, plural: String = "${singular}s"): String =
    "$count ${if (count == 1) singular else plural}"
