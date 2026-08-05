package com.example.openhome.ui.main

internal data class RemoteButtonDefinition(val command: String, val label: String)

internal val REMOTE_BUTTON_ROWS: List<List<RemoteButtonDefinition?>> =
  listOf(
    listOf(
      RemoteButtonDefinition(command = "power", label = "Power"),
      RemoteButtonDefinition(command = "input", label = "Input"),
    ),
    listOf(
      RemoteButtonDefinition(command = "volume-up", label = "Volume +"),
      RemoteButtonDefinition(command = "mute", label = "Mute"),
      RemoteButtonDefinition(command = "volume-down", label = "Volume -"),
    ),
    listOf(
      RemoteButtonDefinition(command = "channel-up", label = "Channel +"),
      RemoteButtonDefinition(command = "guide", label = "Guide"),
      RemoteButtonDefinition(command = "channel-down", label = "Channel -"),
    ),
    listOf(
      RemoteButtonDefinition(command = "home", label = "Home"),
      RemoteButtonDefinition(command = "settings", label = "Settings"),
      RemoteButtonDefinition(command = "info", label = "Info"),
    ),
    listOf(null, RemoteButtonDefinition(command = "up", label = "Up"), null),
    listOf(
      RemoteButtonDefinition(command = "left", label = "Left"),
      RemoteButtonDefinition(command = "ok", label = "OK"),
      RemoteButtonDefinition(command = "right", label = "Right"),
    ),
    listOf(
      RemoteButtonDefinition(command = "back", label = "Back"),
      RemoteButtonDefinition(command = "down", label = "Down"),
      RemoteButtonDefinition(command = "exit", label = "Exit"),
    ),
    listOf(
      RemoteButtonDefinition(command = "1", label = "1"),
      RemoteButtonDefinition(command = "2", label = "2"),
      RemoteButtonDefinition(command = "3", label = "3"),
    ),
    listOf(
      RemoteButtonDefinition(command = "4", label = "4"),
      RemoteButtonDefinition(command = "5", label = "5"),
      RemoteButtonDefinition(command = "6", label = "6"),
    ),
    listOf(
      RemoteButtonDefinition(command = "7", label = "7"),
      RemoteButtonDefinition(command = "8", label = "8"),
      RemoteButtonDefinition(command = "9", label = "9"),
    ),
    listOf(null, RemoteButtonDefinition(command = "0", label = "0"), null),
    listOf(
      RemoteButtonDefinition(command = "hdmi-1", label = "HDMI 1"),
      RemoteButtonDefinition(command = "hdmi-2", label = "HDMI 2"),
      RemoteButtonDefinition(command = "hdmi-3", label = "HDMI 3"),
    ),
  )

internal val REMOTE_BUTTONS: List<RemoteButtonDefinition> = REMOTE_BUTTON_ROWS.flatten().filterNotNull()

internal val REMOTE_BUTTON_COMMANDS: Set<String> = REMOTE_BUTTONS.mapTo(linkedSetOf()) { it.command }
