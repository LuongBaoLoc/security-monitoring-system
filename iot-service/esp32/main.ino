// ESP32 telemetry/command example. Install PubSubClient and ArduinoJson libraries.
#include <WiFi.h>
#include <PubSubClient.h>
#include <ArduinoJson.h>

const char* WIFI_SSID = "CHANGE_ME";
const char* WIFI_PASSWORD = "CHANGE_ME";
const char* MQTT_HOST = "192.168.1.10"; // Set to the broker address reachable by the ESP32.
const uint16_t MQTT_PORT = 1883;
const char* DEVICE_ID = "ESP32_DEMO_001";
const int PIR_PIN = 27;
const int MQ2_PIN = 34;
const int RELAY_PIN = 26;
const int BUZZER_PIN = 25;
const int LOCK_PIN = 33;

WiFiClient wifiClient;
PubSubClient mqtt(wifiClient);
unsigned long lastTelemetry = 0;
String topic(const char* suffix) { return String("security/device/") + DEVICE_ID + "/" + suffix; }

void publishAck(const char* commandId, const char* action, const char* result) {
  StaticJsonDocument<256> doc;
  doc["command_id"] = commandId; doc["device_id"] = DEVICE_ID;
  doc["action"] = action; doc["status"] = result;
  char payload[256]; serializeJson(doc, payload, sizeof(payload));
  mqtt.publish(topic("ack").c_str(), payload);
}

void onMessage(char* receivedTopic, byte* payload, unsigned int length) {
  StaticJsonDocument<384> doc;
  if (deserializeJson(doc, payload, length)) return;
  const char* action = doc["action"] | "";
  const char* commandId = doc["command_id"] | "";
  bool accepted = true;
  if (!strcmp(action, "relay_on")) digitalWrite(RELAY_PIN, HIGH);
  else if (!strcmp(action, "relay_off")) digitalWrite(RELAY_PIN, LOW);
  else if (!strcmp(action, "buzzer_on")) digitalWrite(BUZZER_PIN, HIGH);
  else if (!strcmp(action, "buzzer_off")) digitalWrite(BUZZER_PIN, LOW);
  else if (!strcmp(action, "lock")) digitalWrite(LOCK_PIN, HIGH);
  else if (!strcmp(action, "unlock")) digitalWrite(LOCK_PIN, LOW);
  else accepted = false;
  publishAck(commandId, action, accepted ? "accepted" : "rejected");
}

void connectMqtt() {
  while (!mqtt.connected()) {
    String clientId = String(DEVICE_ID) + "-" + String((uint32_t)ESP.getEfuseMac(), HEX);
    if (mqtt.connect(clientId.c_str())) {
      mqtt.subscribe(topic("command").c_str());
      StaticJsonDocument<128> state; state["device_id"] = DEVICE_ID; state["status"] = "online";
      char payload[128]; serializeJson(state, payload, sizeof(payload));
      mqtt.publish(topic("status").c_str(), payload, true);
    } else delay(3000);
  }
}

void setup() {
  pinMode(PIR_PIN, INPUT); pinMode(RELAY_PIN, OUTPUT); pinMode(BUZZER_PIN, OUTPUT); pinMode(LOCK_PIN, OUTPUT);
  digitalWrite(RELAY_PIN, LOW); digitalWrite(BUZZER_PIN, LOW); digitalWrite(LOCK_PIN, LOW);
  Serial.begin(115200);
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
  while (WiFi.status() != WL_CONNECTED) delay(500);
  mqtt.setServer(MQTT_HOST, MQTT_PORT); mqtt.setCallback(onMessage);
}

void loop() {
  if (!mqtt.connected()) connectMqtt();
  mqtt.loop();
  if (millis() - lastTelemetry >= 5000) {
    lastTelemetry = millis();
    StaticJsonDocument<256> doc;
    doc["device_id"] = DEVICE_ID; doc["pir"] = digitalRead(PIR_PIN) == HIGH;
    doc["mq2"] = analogRead(MQ2_PIN); doc["relay"] = digitalRead(RELAY_PIN) == HIGH;
    doc["buzzer"] = digitalRead(BUZZER_PIN) == HIGH; doc["lock"] = digitalRead(LOCK_PIN) == HIGH;
    char payload[256]; serializeJson(doc, payload, sizeof(payload));
    mqtt.publish(topic("sensor").c_str(), payload);
  }
}
