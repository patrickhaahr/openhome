#pragma once

#include <Arduino.h>

void initIrCommands();
void printIrCommandHelp(Stream &out);
bool handleSerialIrCommand(char command, Stream &out);

enum class IrCommandResult {
  Sent,
  UnknownRemote,
  UnknownCommand,
};

IrCommandResult handleRemoteIrCommand(const String &remote,
                                      const String &command, Stream &out);
String irRemoteListJson();
