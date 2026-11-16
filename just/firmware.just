# Compile both firmware targets in one pass
[group('firmware')]
firmware-check: ir-build switchbot-build

# Compile the ESP32 IR remote firmware
[group('firmware')]
[working-directory: 'firmware/ir-remote']
ir-build:
    pio run

# Upload the IR remote firmware to the automatically detected serial port
[group('firmware')]
[working-directory: 'firmware/ir-remote']
ir-upload:
    pio run --target upload

# Open the IR remote's 115200-baud serial monitor
[group('firmware')]
[working-directory: 'firmware/ir-remote']
ir-monitor:
    pio device monitor

# List serial devices visible to PlatformIO
[group('firmware')]
[working-directory: 'firmware/ir-remote']
ir-devices:
    pio device list

# Remove IR remote build artifacts
[group('firmware')]
[working-directory: 'firmware/ir-remote']
ir-clean:
    pio run --target clean

switchbot-port := env_var_or_default('UPLOAD_PORT', '/dev/ttyUSB0')
switchbot-core-dir := '.platformio'

# Compile the ESP32 SwitchBot firmware
[group('firmware')]
[working-directory: 'firmware/switchbot']
switchbot-build:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio run

# Upload the SwitchBot firmware to UPLOAD_PORT (default /dev/ttyUSB0)
[group('firmware')]
[working-directory: 'firmware/switchbot']
switchbot-flash:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio run --target upload --upload-port {{switchbot-port}}

# Open the SwitchBot serial monitor on UPLOAD_PORT
[group('firmware')]
[working-directory: 'firmware/switchbot']
switchbot-monitor:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio device monitor --port {{switchbot-port}}

# Remove SwitchBot build artifacts
[group('firmware')]
[working-directory: 'firmware/switchbot']
switchbot-clean:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio run --target clean
