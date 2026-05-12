package com.example.openhome.ui.main

internal data class RemoteButtonDefinition(val command: String, val label: String)

internal val REMOTE_BUTTON_ROWS =
  listOf(
    listOf(
      RemoteButtonDefinition(command = "power", label = "Power"),
      RemoteButtonDefinition(command = "bluetooth", label = "Bluetooth"),
      RemoteButtonDefinition(command = "optical", label = "Optical"),
    ),
    listOf(
      RemoteButtonDefinition(command = "mute", label = "Mute"),
      RemoteButtonDefinition(command = "volume-down", label = "Volume -"),
      RemoteButtonDefinition(command = "volume-up", label = "Volume +"),
    ),
  )

internal val REMOTE_BUTTONS: List<RemoteButtonDefinition> = REMOTE_BUTTON_ROWS.flatten()

internal val REMOTE_BUTTON_COMMANDS: Set<String> = REMOTE_BUTTONS.mapTo(linkedSetOf()) { it.command }
