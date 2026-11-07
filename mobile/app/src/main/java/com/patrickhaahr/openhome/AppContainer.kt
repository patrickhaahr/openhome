package com.patrickhaahr.openhome

import android.content.Context
import com.patrickhaahr.openhome.api.newOpenHomeHttpClient
import com.patrickhaahr.openhome.fitness.ActiveWorkoutRepository
import com.patrickhaahr.openhome.fitness.ExerciseLibrary
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.fitness.ProfileRecords
import com.patrickhaahr.openhome.fitness.RestAlarm
import com.patrickhaahr.openhome.fitness.RestTimers
import com.patrickhaahr.openhome.fitness.WorkoutCorrections
import com.patrickhaahr.openhome.setup.ApiKeyCipher
import com.patrickhaahr.openhome.setup.SetupService
import com.patrickhaahr.openhome.storage.OpenHomeDatabase
import com.patrickhaahr.openhome.sync.FitnessOutbox
import com.patrickhaahr.openhome.sync.FitnessSync
import com.patrickhaahr.openhome.sync.FitnessSyncScheduler
import com.patrickhaahr.openhome.sync.FitnessSyncStatus
import com.patrickhaahr.openhome.sync.WifiNetworks
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

/** The app's single instances, wired by hand. */
class AppContainer(context: Context) {
    private val database = OpenHomeDatabase.open(context)
    private val httpClient = newOpenHomeHttpClient()
    private val outbox = FitnessOutbox(database)

    /** Outlives every screen, so leaving one cannot cancel a save it just started. */
    val saveScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    // Household commands have a short total deadline and must never be automatically retried.
    val homeHttpClient = httpClient.newBuilder()
        .callTimeout(5, java.util.concurrent.TimeUnit.SECONDS)
        .retryOnConnectionFailure(false)
        .build()

    val fitness = FitnessRepository(database.fitnessDao())
    val setup = SetupService(database, ApiKeyCipher(), httpClient)
    val wifi = WifiNetworks(context)
    val syncScheduler = FitnessSyncScheduler(context, database.outboxDao())
    val sync = FitnessSync(outbox, setup, wifi, httpClient)
    val syncStatus = FitnessSyncStatus(database.outboxDao(), wifi, sync.sending, outbox, syncScheduler)
    val restTimers = RestTimers(database, RestAlarm(context))
    val activeWorkout = ActiveWorkoutRepository(database, outbox, syncScheduler, restTimers)
    val exerciseLibrary = ExerciseLibrary(database, outbox, syncScheduler)
    val corrections = WorkoutCorrections(database, outbox, syncScheduler)
    val profileRecords = ProfileRecords(database, outbox, syncScheduler)
}
