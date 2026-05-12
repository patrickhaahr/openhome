package com.example.openhome

import android.content.Context
import com.example.openhome.data.DefaultIrRepository
import com.example.openhome.data.DefaultOpenHomeClient
import com.example.openhome.data.DefaultSetupRepository
import com.example.openhome.data.IrRepository
import com.example.openhome.data.OpenHomeClient
import com.example.openhome.data.SetupRepository

class OpenHomeAppContainer(context: Context) {
  val setupRepository: SetupRepository = DefaultSetupRepository(context.applicationContext)
  val openHomeClient: OpenHomeClient = DefaultOpenHomeClient(setupRepository = setupRepository)
  val irRepository: IrRepository = DefaultIrRepository(openHomeClient = openHomeClient)
}
