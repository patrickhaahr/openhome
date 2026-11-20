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
  seconds. Frequent power queries keep the link alive without reasserting the
  intended lighting selection.
- **Disconnect only when Unavailable.** A healthy link persists between
  exchanges. Disconnecting after Off races the controller's advertising
  restart when TV Sync follows immediately.
- **Discovery after host startup.** BlueZ has no cached device after the host
  starts, so the first exchange runs discovery.
- **New links need recovery.** A newly established link triggers
  current-target recovery even when reads succeed.
- **Writes count when sent.** A characteristic write takes effect the moment it
  is sent. Bookkeeping records it even if the exchange was superseded, and
  completed writes stay acknowledged when readback fails or times out.
  Supersession never rewinds an exchange that already wrote. A receipt or a
  readback alone never counts as acceptance.
- **Interrupted uploads.** A multi-frame scene upload checks that its target
  is still current before every frame, so an obsolete upload sends no later
  frames. A fresh `A3` header resets an interrupted upload on the device.
- **Brightness is a command value.** It is the controller's command value, not
  a photometric calibration.
- **Independent test vectors.** Effect tests compare against an independent
  H605C catalogue capture (Aurora, effect 215, sent as five data frames), not
  against output of the encoder under test.
