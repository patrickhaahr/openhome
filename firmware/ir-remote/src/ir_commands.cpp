#include "ir_commands.h"

#include <IRremoteESP8266.h>
#include <IRsend.h>

#include "commands/edifier/commands.h"
#include "commands/ir_command.h"
#include "commands/lg_tv/commands.h"

namespace {

constexpr uint16_t IR_LED_PIN = 4;

IRsend irsend(IR_LED_PIN);

struct CommandSet {
  const char *remoteName;
  const IrCommand *commands;
  size_t count;
};

const CommandSet kCommandSets[] = {
    {"edifier", kEdifierCommands, kEdifierCommandCount},
    {"lgtv", kLgTvCommands, kLgTvCommandCount},
};

const IrCommand *findBySerialKey(char command) {
  for (const CommandSet &set : kCommandSets) {
    for (size_t commandIndex = 0; commandIndex < set.count; commandIndex++) {
      const IrCommand &candidate = set.commands[commandIndex];
      if (candidate.serialKey != '\0' && candidate.serialKey == command) {
        return &candidate;
      }
    }
  }

  return nullptr;
}

const CommandSet *findByRemoteName(const String &remote) {
  for (const CommandSet &set : kCommandSets) {
    if (remote == set.remoteName) {
      return &set;
    }
  }

  return nullptr;
}

const IrCommand *findByName(const CommandSet &set, const String &command) {
  for (size_t commandIndex = 0; commandIndex < set.count; commandIndex++) {
    const IrCommand &candidate = set.commands[commandIndex];
    if (command.equalsIgnoreCase(candidate.name)) {
      return &candidate;
    }
  }

  return nullptr;
}

bool runCommand(const IrCommand *command, Stream &out) {
  if (command == nullptr) {
    return false;
  }

  out.print("Sending ");
  out.println(command->description);
  for (uint8_t i = 0; i < command->transmissionCount; i++) {
    irsend.sendNEC(command->necCode, 32);
    if (i + 1 < command->transmissionCount) {
      delay(command->gapMs);
    }
  }
  return true;
}

}  // namespace

void initIrCommands() {
  irsend.begin();
}

void printIrCommandHelp(Stream &out) {
  out.println("Commands:");
  for (const CommandSet &set : kCommandSets) {
    for (size_t commandIndex = 0; commandIndex < set.count; commandIndex++) {
      const IrCommand &command = set.commands[commandIndex];
      if (command.serialKey != '\0') {
        out.print(command.serialKey);
        out.print(" = ");
      }
      out.print(command.description);
      out.print(" (");
      out.print(command.name);
      out.println(")");
    }
  }
}

bool handleSerialIrCommand(char command, Stream &out) {
  return runCommand(findBySerialKey(command), out);
}

IrCommandResult handleRemoteIrCommand(const String &remote,
                                      const String &command, Stream &out) {
  const CommandSet *set = findByRemoteName(remote);
  if (set == nullptr) {
    return IrCommandResult::UnknownRemote;
  }

  const IrCommand *irCommand = findByName(*set, command);
  if (irCommand == nullptr) {
    return IrCommandResult::UnknownCommand;
  }

  runCommand(irCommand, out);
  return IrCommandResult::Sent;
}

String irRemoteListJson() {
  String response;
  response.reserve(512);
  response += "{\"remotes\":{";
  bool firstSet = true;
  for (const CommandSet &set : kCommandSets) {
    if (!firstSet) {
      response += ',';
    }
    firstSet = false;
    response += '\"';
    response += set.remoteName;
    response += "\":[";
    for (size_t commandIndex = 0; commandIndex < set.count; commandIndex++) {
      if (commandIndex > 0) {
        response += ',';
      }

      response += '\"';
      response += set.commands[commandIndex].name;
      response += '\"';
    }
    response += ']';
  }
  response += "}}";

  return response;
}
