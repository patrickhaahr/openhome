package com.patrickhaahr.openhome

import android.content.Context
import com.patrickhaahr.openhome.api.newOpenHomeHttpClient
import com.patrickhaahr.openhome.fitness.FitnessRepository
import com.patrickhaahr.openhome.setup.ApiKeyCipher
import com.patrickhaahr.openhome.setup.SetupService
import com.patrickhaahr.openhome.storage.OpenHomeDatabase

/** The app's single instances, wired by hand. */
class AppContainer(context: Context) {
    private val database = OpenHomeDatabase.open(context)

    val fitness = FitnessRepository(database.fitnessDao())
    val setup = SetupService(database, ApiKeyCipher(), newOpenHomeHttpClient())
}
