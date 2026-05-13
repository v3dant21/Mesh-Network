#include <WiFi.h>
#include <WiFiClient.h>
#include <WiFiServer.h>
#include <vector>

// --- Configuration ---
const char* ssid = "YOUR_SSID";
const char* password = "YOUR_PASSWORD";
const int port = 9090;

WiFiServer server(port);

struct Node {
    WiFiClient client;
    uint32_t id;
    bool registered = false;
};

std::vector<Node> nodes;

void setup() {
    Serial.begin(115200);
    delay(1000);

    Serial.println("\n--- MESHCOM ESP32 RELAY ---");
    
    // Connect to Wi-Fi
    Serial.printf("Connecting to %s ", ssid);
    WiFi.begin(ssid, password);
    while (WiFi.status() != WL_CONNECTED) {
        delay(500);
        Serial.print(".");
    }
    Serial.println("\nWiFi Connected!");
    Serial.print("IP Address: ");
    Serial.println(WiFi.localIP());

    // Start Server
    server.begin();
    Serial.printf("Relay listening on port %d\n", port);
}

// Function to read Big-Endian uint32
uint32_t readBE32(WiFiClient& client) {
    uint8_t buf[4];
    if (client.readBytes(buf, 4) != 4) return 0;
    return (uint32_t)buf[0] << 24 | (uint32_t)buf[1] << 16 | (uint32_t)buf[2] << 8 | (uint32_t)buf[3];
}

void loop() {
    // 1. Handle New Connections
    WiFiClient newClient = server.available();
    if (newClient) {
        Serial.println("[SYSTEM] New node connected.");
        nodes.push_back({newClient, 0, false});
    }

    // 2. Process All Nodes
    for (auto it = nodes.begin(); it != nodes.end(); ) {
        if (!it->client.connected()) {
            if (it->registered) {
                Serial.printf("[RELAY] Node %u disconnected.\n", it->id);
            } else {
                Serial.println("[SYSTEM] Unregistered client disconnected.");
            }
            it = nodes.erase(it);
            continue;
        }

        if (it->client.available() >= 4) {
            // Read framing length (Big Endian)
            uint32_t packetLen = readBE32(it->client);
            
            if (packetLen > 0 && packetLen < 10000) { // Safety check
                uint8_t* buffer = (uint8_t*)malloc(packetLen);
                if (buffer) {
                    size_t n = it->client.readBytes(buffer, packetLen);
                    if (n == packetLen) {
                        // Bincode is Little Endian
                        // Offset 0-3: packet ID
                        // Offset 4-7: from ID
                        // Offset 8-11: to ID
                        uint32_t fromId = *(uint32_t*)(buffer + 4);
                        uint32_t toId = *(uint32_t*)(buffer + 8);

                        // Registration
                        if (!it->registered) {
                            it->id = fromId;
                            it->registered = true;
                            Serial.printf("[RELAY] Registered Node %u\n", fromId);
                        }

                        // Routing
                        if (toId == 0) {
                            // Broadcast
                            Serial.printf("[ROUTING] Broadcast from %u\n", fromId);
                            for (auto& other : nodes) {
                                if (other.registered && other.id != fromId) {
                                    // Send framing
                                    uint8_t lenBuf[4];
                                    lenBuf[0] = (packetLen >> 24) & 0xFF;
                                    lenBuf[1] = (packetLen >> 16) & 0xFF;
                                    lenBuf[2] = (packetLen >> 8) & 0xFF;
                                    lenBuf[3] = packetLen & 0xFF;
                                    other.client.write(lenBuf, 4);
                                    // Send payload
                                    other.client.write(buffer, packetLen);
                                }
                            }
                        } else {
                            // Direct Route
                            bool found = false;
                            for (auto& target : nodes) {
                                if (target.registered && target.id == toId) {
                                    Serial.printf("[ROUTING] Direct: %u -> %u\n", fromId, toId);
                                    uint8_t lenBuf[4];
                                    lenBuf[0] = (packetLen >> 24) & 0xFF;
                                    lenBuf[1] = (packetLen >> 16) & 0xFF;
                                    lenBuf[2] = (packetLen >> 8) & 0xFF;
                                    lenBuf[3] = packetLen & 0xFF;
                                    target.client.write(lenBuf, 4);
                                    target.client.write(buffer, packetLen);
                                    found = true;
                                    break;
                                }
                            }
                            if (!found) {
                                Serial.printf("[ROUTING] Target %u not found\n", toId);
                            }
                        }
                    }
                    free(buffer);
                }
            }
        }
        ++it;
    }

    delay(1); // Small yield
}
