package com.example.openhome.ui.main

internal data class HomeRemoteControlDefinition(val command: String, val label: String)

internal val HOME_REMOTE_CONTROLS =
  listOf(
    HomeRemoteControlDefinition(command = "bluetooth", label = "Bluetooth"),
    HomeRemoteControlDefinition(command = "optical", label = "Optical"),
  )

internal val HOME_REMOTE_CONTROL_COMMANDS: Set<String> = HOME_REMOTE_CONTROLS.mapTo(linkedSetOf()) { it.command }
