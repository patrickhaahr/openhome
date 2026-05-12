package com.example.openhome

import android.content.Context
import com.example.openhome.data.DefaultSetupRepository
import com.example.openhome.data.SetupRepository

class OpenHomeAppContainer(context: Context) {
  val setupRepository: SetupRepository = DefaultSetupRepository(context.applicationContext)
}
