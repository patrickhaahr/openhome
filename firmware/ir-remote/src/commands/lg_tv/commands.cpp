#include "commands/lg_tv/commands.h"

// LG TVs use NEC 32-bit frames for these standard remote functions.
// Sources: https://github.com/IvanGlinkin/Default-IR-vendors-samples/tree/main/LG/TV
//          https://github.com/trodemaster/rc/blob/main/reference/LG_C4_IR_Codes.txt
const IrCommand kLgTvCommands[] = {
    {'P', "power", "LG TV Power", 0x20DF10EF, 1, 0},
    {'I', "input", "LG TV Input", 0x20DFD02F, 1, 0},
    {'U', "volume-up", "LG TV Volume Up", 0x20DF40BF, 1, 0},
    {'D', "volume-down", "LG TV Volume Down", 0x20DFC03F, 1, 0},
    {'M', "mute", "LG TV Mute", 0x20DF906F, 1, 0},
    {'H', "home", "LG TV Home", 0x20DF3EC1, 1, 0},
    {'\0', "settings", "LG TV Settings", 0x20DFC23D, 1, 0},
    {'\0', "up", "LG TV Up", 0x20DF02FD, 1, 0},
    {'\0', "down", "LG TV Down", 0x20DF827D, 1, 0},
    {'\0', "left", "LG TV Left", 0x20DFE01F, 1, 0},
    {'\0', "right", "LG TV Right", 0x20DF609F, 1, 0},
    {'\0', "ok", "LG TV OK", 0x20DF22DD, 1, 0},
    {'\0', "back", "LG TV Back", 0x20DF14EB, 1, 0},
    {'\0', "exit", "LG TV Exit", 0x20DFDA25, 1, 0},
    {'\0', "channel-up", "LG TV Channel Up", 0x20DF00FF, 1, 0},
    {'\0', "channel-down", "LG TV Channel Down", 0x20DF807F, 1, 0},
    {'\0', "guide", "LG TV Guide", 0x20DFD52A, 1, 0},
    {'\0', "info", "LG TV Info", 0x20DF55AA, 1, 0},
    {'\0', "0", "LG TV 0", 0x20DF08F7, 1, 0},
    {'\0', "1", "LG TV 1", 0x20DF8877, 1, 0},
    {'\0', "2", "LG TV 2", 0x20DF48B7, 1, 0},
    {'\0', "3", "LG TV 3", 0x20DFC837, 1, 0},
    {'\0', "4", "LG TV 4", 0x20DF28D7, 1, 0},
    {'\0', "5", "LG TV 5", 0x20DFA857, 1, 0},
    {'\0', "6", "LG TV 6", 0x20DF6897, 1, 0},
    {'\0', "7", "LG TV 7", 0x20DFE817, 1, 0},
    {'\0', "8", "LG TV 8", 0x20DF18E7, 1, 0},
    {'\0', "9", "LG TV 9", 0x20DF9867, 1, 0},
    {'\0', "hdmi-1", "LG TV HDMI 1", 0x20DF738C, 1, 0},
    {'\0', "hdmi-2", "LG TV HDMI 2", 0x20DF33CC, 1, 0},
    {'\0', "hdmi-3", "LG TV HDMI 3", 0x20DF9768, 1, 0},
};

const size_t kLgTvCommandCount =
    sizeof(kLgTvCommands) / sizeof(kLgTvCommands[0]);
