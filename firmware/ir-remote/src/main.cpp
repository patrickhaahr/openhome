#include <Arduino.h>

#include "ir_commands.h"
#include "wifi_control.h"

void setup() {
  Serial.begin(115200);
  initIrCommands();
  setupWifiControl();

  Serial.println("Ready.");
  printIrCommandHelp(Serial);
}

void loop() {
  handleWifiControl();

  if (Serial.available()) {
    char command = Serial.read();
    if (!handleSerialIrCommand(command, Serial)) {
      Serial.println("Unknown command.");
      printIrCommandHelp(Serial);
    }
  }
}
