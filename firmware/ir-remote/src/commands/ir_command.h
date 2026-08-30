#pragma once

#include <Arduino.h>

struct IrCommand {
  char serialKey;
  const char *name;
  const char *description;
  uint32_t necCode;
  uint8_t transmissionCount;
  uint16_t gapMs;
};
