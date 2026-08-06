#include <Arduino.h>
#include <ESP32Servo.h>
#include <WebServer.h>
#include <WiFi.h>

namespace {

constexpr uint8_t kServoPin = 4;
constexpr int kNeutralAngle = 90;
constexpr int kOnAngle = 45;
constexpr int kOffAngle = 135;
static_assert(kOnAngle < kNeutralAngle);
static_assert(kOffAngle > kNeutralAngle);
constexpr uint16_t kPressDurationMs = 500;
constexpr uint16_t kHttpPort = 80;

Servo servo;
WebServer server(kHttpPort);

void pressSwitch(int angle) {
  servo.write(angle);
  delay(kPressDurationMs);
  servo.write(kNeutralAngle);
  delay(kPressDurationMs);
}

void handleLightCommand(int angle, const char* state) {
  pressSwitch(angle);
  String response = "{\"message\":\"Light switched ";
  response += state;
  response += "\"}";
  server.send(200, "application/json", response);
}

void connectToWifi() {
  WiFi.mode(WIFI_STA);
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);

  Serial.print("Connecting to Wi-Fi");
  uint8_t attempts = 0;
  while (WiFi.status() != WL_CONNECTED && attempts < 40) {
    delay(250);
    Serial.print('.');
    attempts++;
  }
  Serial.println();
}

void handleNotFound() {
  server.send(404, "application/json", "{\"error\":\"Not found\"}");
}

}  // namespace

void setup() {
  Serial.begin(115200);

  servo.setPeriodHertz(50);
  servo.attach(kServoPin, 500, 2400);
  servo.write(kNeutralAngle);
  delay(kPressDurationMs);

  connectToWifi();
  if (WiFi.status() == WL_CONNECTED) {
    Serial.print("Connected to Wi-Fi. IP: ");
    Serial.println(WiFi.localIP());
  } else {
    Serial.println("Failed to connect to Wi-Fi. Restart to try again.");
  }

  server.on("/", HTTP_GET, []() {
    server.send(200, "application/json",
                "{\"message\":\"Switchbot ready. POST /lights/on or "
                "/lights/off.\"}");
  });
  server.on("/lights/on", HTTP_POST,
            []() { handleLightCommand(kOnAngle, "on"); });
  server.on("/lights/off", HTTP_POST,
            []() { handleLightCommand(kOffAngle, "off"); });
  server.onNotFound(handleNotFound);
  server.begin();

  Serial.println("HTTP server listening on port 80.");
}

void loop() {
  server.handleClient();
}
