# Backlight device quirks

ADR 0011 chose local Bluetooth control of the Govee H605C, and
`docs/backlight.md` describes the delivery model. This record keeps the device
and BlueZ behaviour the code must work around, which only hardware testing
showed.

- **Whole-strip RGBIC encoding.** The H605C needs the RGBIC whole-strip
  variant for both manual colour and scenes. The generic `33 05 02` colour
  frame and the generic RGB scene variant are acknowledged but have no effect.
  An acknowledgement is therefore never proof that a colour applied.
- **Idle link.** The strip drops an idle Bluetooth link after about twenty
  seconds, which is why the two-second probe in `docs/backlight.md` includes a
  power query.
- **Disconnect only when Unavailable.** A healthy link persists between
  exchanges. Disconnecting after Off races the controller's advertising
  restart when TV Sync follows immediately.
- **Discovery after host startup.** BlueZ has no cached device after the host
  starts, so every exchange starts discovery and waits for the strip whenever
  BlueZ does not list it yet.
- **Writes count when sent.** A characteristic write takes effect the moment it
  is sent. An exchange whose writes all completed stays acknowledged when a
  newer revision arrived meanwhile, or when readback fails. An exchange that
  stops part-way as superseded records nothing and is never rewound. A readback
  alone never counts as acceptance.
- **Brightness is a command value.** It is the controller's command value, not
  a photometric calibration.
- **Independent test vectors.** Effect tests compare against an independent
  H605C catalogue capture (Aurora, effect 215, sent as five data frames), not
  against output of the encoder under test.
