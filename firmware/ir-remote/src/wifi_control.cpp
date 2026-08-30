#include "wifi_control.h"

#include <Arduino.h>
#include <WebServer.h>
#include <WiFi.h>

#include "ir_commands.h"

namespace {

constexpr uint16_t kHttpPort = 80;

WebServer server(kHttpPort);

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

void handleRoot() {
  server.send(200, "application/json",
              "{\"message\":\"IR remote ready. Use GET /remotes and POST "
              "/remotes/{remote}.\"}");
}

void handleRemotes() {
  server.send(200, "application/json", irRemoteListJson());
}

void handleRemoteMethodNotAllowed() {
  server.sendHeader("Allow", "POST");
  server.send(405, "application/json",
              "{\"error\":\"Method not allowed. Use POST.\"}");
}

void handleRemoteCommand(const String &remote) {
  if (!server.hasArg("command")) {
    server.send(400, "application/json",
                "{\"error\":\"Missing form field: command\"}");
    return;
  }

  const String command = server.arg("command");
  switch (handleRemoteIrCommand(remote, command, Serial)) {
    case IrCommandResult::UnknownRemote:
      server.send(404, "application/json",
                  "{\"error\":\"Unknown remote\"}");
      return;
    case IrCommandResult::UnknownCommand:
      server.send(404, "application/json",
                  "{\"error\":\"Unknown command\"}");
      return;
    case IrCommandResult::Sent:
      break;
  }

  String response = "{\"message\":\"Sent command: ";
  response += command;
  response += "\"}";
  server.send(200, "application/json", response);
}

void handleNotFound() {
  const String uri = server.uri();
  if (uri == "/remotes/edifier" || uri == "/remotes/lgtv") {
    handleRemoteMethodNotAllowed();
    return;
  }

  if (uri == "/" || uri == "/remotes") {
    server.sendHeader("Allow", "GET");
    server.send(405, "application/json",
                "{\"error\":\"Method not allowed. Use GET.\"}");
    return;
  }

  server.send(404, "application/json", "{\"error\":\"Not found\"}");
}

}  // namespace

void setupWifiControl() {
  connectToWifi();

  if (WiFi.status() == WL_CONNECTED) {
    Serial.print("Connected to Wi-Fi. IP: ");
    Serial.println(WiFi.localIP());
  } else {
    Serial.println("Failed to connect to Wi-Fi.");
  }

  server.on("/", HTTP_GET, handleRoot);
  server.on("/remotes", HTTP_GET, handleRemotes);
  server.on("/remotes/edifier", HTTP_POST,
            []() { handleRemoteCommand("edifier"); });
  server.on("/remotes/lgtv", HTTP_POST,
            []() { handleRemoteCommand("lgtv"); });
  server.onNotFound(handleNotFound);
  server.begin();

  Serial.print("HTTP server listening on port ");
  Serial.println(kHttpPort);
}

void handleWifiControl() {
  server.handleClient();
}
