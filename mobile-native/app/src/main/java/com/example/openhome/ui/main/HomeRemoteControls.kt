package com.example.openhome.ui.main

internal data class HomeRemoteControlDefinition(val command: String, val label: String)

internal val HOME_REMOTE_CONTROL_ROWS =
  listOf(
    listOf(
      HomeRemoteControlDefinition(command = "power", label = "Power"),
      HomeRemoteControlDefinition(command = "bluetooth", label = "Bluetooth"),
      HomeRemoteControlDefinition(command = "optical", label = "Optical"),
    ),
    listOf(
      HomeRemoteControlDefinition(command = "mute", label = "Mute"),
      HomeRemoteControlDefinition(command = "volume-down", label = "Volume -"),
      HomeRemoteControlDefinition(command = "volume-up", label = "Volume +"),
    ),
  )

internal val HOME_REMOTE_CONTROLS = HOME_REMOTE_CONTROL_ROWS.flatten()

internal val HOME_REMOTE_CONTROL_COMMANDS: Set<String> = HOME_REMOTE_CONTROLS.mapTo(linkedSetOf()) { it.command }
