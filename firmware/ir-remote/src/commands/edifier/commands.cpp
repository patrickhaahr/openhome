#include "commands/edifier/commands.h"

const IrCommand kEdifierCommands[] = {
    {'b', "bluetooth", "Edifier Bluetooth", 0x08E7708F, 1, 0},
    {'o', "optical", "Edifier Optical burst", 0x08E7B04F, 8, 120},
    {'m', "mute", "Edifier Mute", 0x08E700FF, 1, 0},
    {'u', "volume-up", "Edifier Volume Up", 0x08E7906F, 1, 0},
    {'d', "volume-down", "Edifier Volume Down", 0x08E730CF, 1, 0},
    {'p', "power", "Edifier Power", 0x08E7807F, 1, 0},
};

const size_t kEdifierCommandCount =
    sizeof(kEdifierCommands) / sizeof(kEdifierCommands[0]);
