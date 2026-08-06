# NodeMCU ESP32 switchbot

An ESP32-WROOM-32D NodeMCU drives an SG90 servo to press the two sides of a normal wall
rocker switch. It joins your Wi-Fi and exposes two HTTP commands.

## Safety first

- Do not remove the wall-switch cover or touch its wiring. This project only
  presses the insulated plastic switch from outside.
- Do not solder while the board is powered. Unplug USB first.
- The capacitor is polarized: its striped `-` leg goes to GND. Reversing it
  can damage it.
- Power the SG90 from 5 V, never from the ESP32's 3.3 V pin.
- Test everything loose on the desk before mounting it to the wall.

## Wiring

| SG90 / capacitor | NodeMCU ESP32-WROOM-32D |
| --- | --- |
| SG90 brown (GND) | GND |
| SG90 red (+5 V) | VIN 5V |
| SG90 orange/yellow (signal) | GPIO4 |
| Capacitor striped `-` leg | GND |
| Capacitor other `+` leg | 5V/VBUS |

The capacitor goes across 5 V and GND, close to the servo connector. Check the
labels printed on your exact board before powering it; clone pin layouts vary.
A decent USB power supply and cable should provide at least 1 A. If the ESP32
restarts when the servo moves, use a separate regulated 5 V supply rated at
1 A or more for the servo and connect that supply's GND to ESP32 GND.

Do not permanently solder first. If your board has header pins, use temporary
jumper leads to prove the wiring and angles. Your 5x7 cm prototype PCB is
solderable perfboard, not a reusable solderless breadboard.

## Build and flash

Create the private Wi-Fi file:

```sh
cp .env.example .env
```

Edit `.env`, then run:

```sh
direnv allow
just switchbot-build
just switchbot-flash
just switchbot-monitor
```

The NodeMCU may enter programming mode automatically. If the upload stays at
`Connecting...`, hold the `BOOT` button. Keep holding it until uploading starts,
then release it. Do not hold `EN`/Reset.

`GPIO4` is the header labelled `GPIO4`/`P4`; it appears on physical row 26 in
the supplied pinout image. It is not GPIO26.

The serial monitor prints the device IP. ESP32 devices support 2.4 GHz Wi-Fi,
not a 5 GHz-only network.

Test from another device on the same Wi-Fi:

```sh
curl -X POST http://DEVICE_IP/lights/on
curl -X POST http://DEVICE_IP/lights/off
```

## Calibrate and mount

1. Leave the servo horn off for the first power-up. The firmware centers the
   shaft at 90 degrees.
2. Unplug USB, fit the horn in a useful centered position, and tighten its
   small screw.
3. Temporarily hold the servo beside the switch. It must press the rocker
   without stalling or forcing it past its normal travel.
4. Adjust `kOnAngle`, `kOffAngle`, and, if needed, `kNeutralAngle` in
   `src/main.cpp`. Start with changes of only 5 degrees.
5. Repeat desk tests before fixing the servo beside the switch with a rigid
   bracket or strong removable mounting tape. Do not mount the PCB bare where
   metal can short it.

The code presses for 500 ms, returns to neutral, and keeps the servo active so
the arm cannot drift away from its configured starting position. The reported
state is only the last requested state; there is no sensor confirming the
light's actual state.

## First soldering tips

Watch a beginner through-hole soldering video and practice on spare perfboard,
not the ESP32. Work with ventilation and eye protection.

1. Set the lead-free iron around 350-370 C and let it fully heat.
2. Tin the clean tip with a small amount of solder; wipe it on brass wool or a
   damp sponge, then tin it again.
3. Put the tip against both the pad and component lead for about one second.
4. Feed a little solder into the heated joint, not directly onto the iron.
5. Remove the solder, then the iron. A joint should be a small smooth cone, not
   a ball. Avoid heating a pad for more than a few seconds.
6. Let the joint cool without movement. Trim the lead and inspect for bridges.
7. Slide heat-shrink onto a wire before soldering it, then shrink it only after
   the joint has cooled. Keep the hot iron away from USB cables and the servo.

Wash your hands after handling solder and do not eat or drink at the bench,
even though your solder is lead-free. Flux fumes still require ventilation.
