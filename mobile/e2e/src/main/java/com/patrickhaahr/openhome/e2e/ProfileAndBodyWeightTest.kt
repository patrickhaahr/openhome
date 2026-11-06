package com.patrickhaahr.openhome.e2e

import java.time.LocalDate
import java.time.format.DateTimeFormatter
import kotlin.math.abs
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

private const val SETTLED_MS = 4_000L
private const val STORED_POLLS = 90
private const val STORED_POLL_MS = 1_000L
private const val SYNCED = "Synced with OpenHome"
private const val WAITING_FOR_WIFI = "Saved on device · Waiting for Wi-Fi"
private const val CANT_REACH = "Saved on device · Can't reach OpenHome, will retry"
private const val PENDING = "Saved on device"
private const val DATE_TAKEN = "OpenHome already has a Body Weight"
private const val ALREADY_RECORDED = "is already recorded as"
private const val DOWNLOADED_REVISION = 4L
private const val MONTHS_PER_YEAR = 12
private val SHOWN_DAY = DateTimeFormatter.ofPattern("EEE, d MMM yyyy")
private val PICKER_DAY = DateTimeFormatter.ofPattern("EEEE, MMMM d, yyyy")

/**
 * The avatar's Profile and Body Weight through the real app, its real database, real process restarts
 * and the emulator's real radios. Only Axum is replaced, at its HTTP boundary.
 */
class ProfileAndBodyWeightTest {
    private val app = AppDriver()
    private val today = LocalDate.now()
    private val lastMonth = today.minusMonths(1).withDayOfMonth(15)
    private val otherDay = lastMonth.plusDays(1)
    private val downloadedWeight = FixtureBodyWeight(1, lastMonth.toString(), 80.5)
    private val downloadedProfile =
        FixtureProfile(178.5, "male", 192, "1994-03-17", clientRevision = DOWNLOADED_REVISION)
    private val api = FakeAxum(bodyWeight = listOf(downloadedWeight), profile = downloadedProfile)

    @Before
    fun completeSetup() {
        app.setWifi(true)
        app.clearAppData()
        app.relaunch()
        app.completeSetup(api)
    }

    @After
    fun restore() {
        app.setWifi(true)
        api.close()
    }

    private fun openProfile() {
        app.tapDescription("Open Profile")
        app.assertVisible("Edit Profile")
    }

    private fun setHeartRate(bpm: String) {
        app.tap("Edit Profile")
        app.enter("Max Heart Rate in bpm (optional)", bpm)
        app.tap("Save Profile")
        app.assertVisible("$bpm bpm")
    }

    private fun recordWeight(kilograms: String, day: LocalDate = today) {
        openWeightForm()
        chooseDay(day)
        app.enter("Weight in kg", kilograms)
        app.tap("Save Body Weight")
    }

    private fun openWeightForm() {
        app.tapBelow("Record Body Weight")
        app.assertVisible("Save Body Weight")
    }

    /** From the form's own current date, today unless this form has already moved it. */
    private fun chooseDay(day: LocalDate, from: LocalDate = today) {
        if (day == from) return
        app.tap(from.format(SHOWN_DAY))
        val months = (day.year - from.year) * MONTHS_PER_YEAR + day.monthValue - from.monthValue
        repeat(abs(months)) { app.tapDescription(if (months < 0) "Change to previous month" else "Change to next month") }
        app.tapTextContaining(day.format(PICKER_DAY))
        app.tap("Set date")
        app.assertVisible(day.format(SHOWN_DAY))
    }

    private fun assertRecorded(day: LocalDate, kilograms: String) {
        app.assertVisibleBelow(day.format(SHOWN_DAY))
        app.assertVisible(kilograms)
    }

    private fun waitUntil(what: String, condition: () -> Boolean) {
        repeat(STORED_POLLS) {
            if (condition()) return
            app.sleepFor(STORED_POLL_MS)
        }
        error(what)
    }

    /** Every Profile field, a new weigh-in, a restart on mobile data; then one delivery of each and the final records. */
    @Test
    fun offlineProfileEditAndWeighInSurviveARestartThenReachAxumOnceOnWifi() {
        app.setWifi(false)
        openProfile()
        app.assertVisible("178.5 cm")
        app.assertVisible("Male")
        app.assertVisible("Thu, 17 Mar 1994")
        app.assertVisible("192 bpm")
        app.assertVisible(SYNCED)

        app.tap("Edit Profile")
        app.assertField("Height in cm (optional)", "178.5")
        app.enter("Height in cm (optional)", "180,5")
        // Tapping the chosen sex clears it, as optional as it is on the API.
        app.tap("Male")
        app.tap("Thu, 17 Mar 1994")
        app.tapTextContaining("Saturday, March 19, 1994")
        app.tap("Set date")
        app.assertVisible("Sat, 19 Mar 1994")
        app.enter("Max Heart Rate in bpm (optional)", "190")
        app.tap("Save Profile")

        // Saved on the device, and already what the app shows.
        app.assertVisible("180.5 cm")
        app.assertVisible("Not set")
        app.assertVisible("Sat, 19 Mar 1994")
        app.assertVisible("190 bpm")
        app.assertVisible(WAITING_FOR_WIFI)

        recordWeight("82,4")
        assertRecorded(today, "82.4 kg")
        assertRecorded(lastMonth, "80.5 kg")

        app.relaunch()
        app.assertOnWorkoutTab()
        app.assertVisible(WAITING_FOR_WIFI)
        openProfile()
        app.assertVisible("180.5 cm")
        app.assertVisible("Not set")
        app.assertVisible("190 bpm")
        app.assertVisible(WAITING_FOR_WIFI)
        assertRecorded(today, "82.4 kg")
        app.sleepFor(SETTLED_MS)
        assertEquals("Mobile data must not send anything", emptyList<Long>(), api.profileDeliveries())
        assertEquals("Mobile data must not send anything", emptyList<String>(), api.bodyWeightDeliveries())

        app.setWifi(true)

        waitUntil("Both changes should arrive") { api.bodyWeightDeliveries().isNotEmpty() && api.profileDeliveries().isNotEmpty() }
        app.assertEventuallyVisible(SYNCED)
        app.assertTextAbsent(PENDING)
        assertEquals(
            FixtureProfile(180.5, null, 190, "1994-03-19", clientRevision = DOWNLOADED_REVISION + 1),
            api.storedProfile(),
        )
        assertEquals(listOf(DOWNLOADED_REVISION + 1), api.profileDeliveries())
        assertEquals(
            listOf(downloadedWeight, FixtureBodyWeight(2, today.toString(), 82.4)),
            api.storedBodyWeight(),
        )
        assertEquals(1, api.bodyWeightDeliveries().size)
    }

    /** The weight commits, its answer is lost, and the retry is the same operation rather than a second entry. */
    @Test
    fun aLostBodyWeightAnswerRetriesTheSameRequestAndLeavesOneRecord() {
        api.loseBodyWeightResponses()
        openProfile()
        recordWeight("82.4")
        app.assertVisibleBelow(today.format(SHOWN_DAY))

        app.assertEventuallyVisible(CANT_REACH)
        assertEquals(2, api.storedBodyWeight().size)

        app.relaunch()
        openProfile()
        waitUntil("A restart should retry") { api.bodyWeightDeliveries().size >= 2 }

        api.healthy()
        app.assertVisibleBelow(today.format(SHOWN_DAY))
        app.assertEventuallyVisible(CANT_REACH)
        app.tap("Sync now")

        app.assertVisibleBelow(today.format(SHOWN_DAY))
        waitUntil("The retry should be acknowledged") { !app.showsTextContaining(PENDING) }
        app.assertVisible("82.4 kg")
        assertEquals("Every delivery is one operation", 1, api.bodyWeightDeliveries().toSet().size)
        assertEquals(
            listOf(downloadedWeight, FixtureBodyWeight(2, today.toString(), 82.4)),
            api.storedBodyWeight(),
        )
    }

    /** The edit commits, its answer is lost, and neither retries nor restarts can bring an older one back. */
    @Test
    fun aLostProfileAnswerRetriesItsRevisionAndANewerEditStaysOnTop() {
        api.loseProfileResponses()
        openProfile()
        setHeartRate("180")
        app.assertEventuallyVisible(CANT_REACH)
        assertEquals(180, api.storedProfile().hrMaxBpm)
        assertEquals("Continues from the downloaded revision", DOWNLOADED_REVISION + 1, api.storedProfile().clientRevision)

        setHeartRate("181")
        waitUntil("The newer edit should commit") { api.storedProfile().hrMaxBpm == 181 }

        app.relaunch()
        openProfile()
        app.assertVisible("181 bpm")
        waitUntil("A restart should retry") { api.profileDeliveries().count { it == DOWNLOADED_REVISION + 2 } >= 2 }
        assertEquals(181, api.storedProfile().hrMaxBpm)

        api.healthy()
        app.tap("Sync now")

        app.assertEventuallyVisible(SYNCED)
        val deliveries = api.profileDeliveries()
        assertEquals(DOWNLOADED_REVISION + 1, deliveries.first())
        assertEquals(DOWNLOADED_REVISION + 2, deliveries.last())
        assertTrue("A retry never goes back to an older revision: $deliveries", deliveries == deliveries.sorted())
        assertEquals(181, api.storedProfile().hrMaxBpm)
        app.assertVisible("181 bpm")
    }

    /** Queued edits are whole records, so a newer one replaces what has not been sent yet. */
    @Test
    fun onlyTheNewestOfSeveralOfflineProfileEditsIsDelivered() {
        app.setWifi(false)
        openProfile()
        setHeartRate("180")
        app.relaunch()
        openProfile()
        setHeartRate("181")

        app.setWifi(true)

        app.assertEventuallyVisible(SYNCED)
        assertEquals(listOf(DOWNLOADED_REVISION + 2), api.profileDeliveries())
        assertEquals(181, api.storedProfile().hrMaxBpm)
        assertEquals("What it did not change stays", 178.5, api.storedProfile().heightCm)
    }

    /** One Body Weight per date, whether downloaded or recorded here: the record stays and nothing is queued. */
    @Test
    fun anAlreadyRecordedDateIsRefusedOnTheDeviceAndItsRecordStays() {
        app.setWifi(false)
        openProfile()
        recordWeight("90", lastMonth)
        app.assertTextContaining(ALREADY_RECORDED)
        app.assertVisible("Save Body Weight")

        // A date free here saves, and is then just as taken, even before it has synced.
        chooseDay(today, from = lastMonth)
        waitUntil("The problem belonged to the other date") { !app.showsTextContaining(ALREADY_RECORDED) }
        app.tap("Save Body Weight")
        assertRecorded(today, "90 kg")
        recordWeight("91")
        app.assertTextContaining(ALREADY_RECORDED)
        app.pressBackButton()

        assertRecorded(today, "90 kg")
        assertRecorded(lastMonth, "80.5 kg")
        app.assertAbsent("91 kg")

        app.setWifi(true)
        waitUntil("The free date should arrive") { api.storedBodyWeight().size == 2 }
        app.sleepFor(SETTLED_MS)
        assertEquals(1, api.bodyWeightDeliveries().size)
        assertEquals(
            listOf(downloadedWeight, FixtureBodyWeight(2, today.toString(), 90.0)),
            api.storedBodyWeight(),
        )
    }

    /**
     * A new entry for a date another client has recorded is no retry: Axum keeps its record, this one
     * waits for the user, and nothing else in the queue waits with it.
     */
    @Test
    fun aDateAxumAlreadyHoldsStaysItsOwnAndWaitsForTheUserWithoutHoldingUpTheQueue() {
        app.setWifi(false)
        openProfile()
        recordWeight("82.4")
        recordWeight("83", otherDay)
        api.putBodyWeight(today.toString(), 79.0)
        app.pressBackButton()
        app.assertOnWorkoutTab()

        app.setWifi(true)

        app.assertEventuallyContaining("OpenHome refused a change")
        waitUntil("The record queued behind it should arrive") {
            api.storedBodyWeight().any { it.date == otherDay.toString() && it.weightKg == 83.0 }
        }
        val refusedDeliveries = api.bodyWeightDeliveries().size

        // Trying again is the same refusal, and neither side's value moves.
        app.tapBelow("Sync now")
        waitUntil("Sync now should try again") { api.bodyWeightDeliveries().size > refusedDeliveries }
        app.assertEventuallyContaining("OpenHome refused a change")
        assertEquals(79.0, api.storedBodyWeight().single { it.date == today.toString() }.weightKg, 0.0)

        app.relaunch()
        app.tapTextContaining("OpenHome refused a change")
        assertRecorded(today, "82.4 kg")
        app.assertTextContaining(DATE_TAKEN)

        app.tap("Use OpenHome's")

        assertRecorded(today, "79 kg")
        waitUntil("The refusal should be settled") { !app.showsTextContaining(DATE_TAKEN) && !app.showsTextContaining(PENDING) }
        app.assertAbsent("82.4 kg")
        assertEquals(
            listOf(
                downloadedWeight,
                FixtureBodyWeight(3, otherDay.toString(), 83.0),
                FixtureBodyWeight(2, today.toString(), 79.0),
            ),
            api.storedBodyWeight(),
        )
        app.pressBackButton()
        app.assertOnWorkoutTab()
        app.assertAbsent("OpenHome refused a change")
        app.assertVisibleBelow(SYNCED)
    }

    /** The API's rules hold before anything is saved, and each nested view gives way to the one before it. */
    @Test
    fun onlyValidValuesSaveAndBackReturnsThroughEachNestedView() {
        app.tap("Home")
        openProfile()
        app.assertNoBottomTabs()

        // Nothing changed, so nothing is queued.
        app.tap("Edit Profile")
        app.tap("Save Profile")
        app.assertVisible("Edit Profile")

        app.tap("Edit Profile")
        app.enter("Height in cm (optional)", "0")
        app.enter("Max Heart Rate in bpm (optional)", "99")
        app.tap("Save Profile")
        app.assertVisible("Enter height in cm above 0, or leave it blank")
        app.assertVisible("Enter a whole number from 100 to 240, or leave it blank")
        app.enter("Height in cm (optional)", "180")
        app.enter("Max Heart Rate in bpm (optional)", "241")
        app.tap("Save Profile")
        // The fixed height's problem clears as this save's answer reaches the screen.
        waitUntil("The fixed height should stop being marked") { !app.showsTextContaining("Enter height in cm") }
        app.assertVisible("Enter a whole number from 100 to 240, or leave it blank")

        // Back drops what was never saved.
        app.pressBackButton()
        app.assertVisible("Edit Profile")
        app.assertVisible("178.5 cm")
        app.assertVisible("192 bpm")

        openWeightForm()
        app.enter("Weight in kg", "0")
        app.tap("Save Body Weight")
        app.assertVisible("Enter a weight in kg above 0")
        app.assertNoBottomTabs()
        app.pressBackButton()
        app.assertVisible("Edit Profile")
        app.assertAbsent("0 kg")

        app.pressBackButton()
        app.assertVisible("Full TV remote")
        app.pressBackButton()
        app.assertOnWorkoutTab()

        app.sleepFor(SETTLED_MS)
        assertEquals(emptyList<Long>(), api.profileDeliveries())
        assertEquals(emptyList<String>(), api.bodyWeightDeliveries())
        assertEquals(downloadedProfile, api.storedProfile())
    }
}
