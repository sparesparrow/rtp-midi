#include <Arduino.h>
#include <WiFi.h>
#include <ESPmDNS.h>
#include <ArduinoOSC.h>
#include <FastLED.h>
#include "board_config.h"

// LED strip configuration
CRGB leds[NUM_LEDS];

// OSC server
OscWiFiServer osc_server;

// FreeRTOS handles
TaskHandle_t networkTaskHandle = NULL;
TaskHandle_t animationTaskHandle = NULL;
QueueHandle_t commandQueue = NULL;

// MIDI note state tracking
struct NoteState {
    bool active;
    uint8_t velocity;
    unsigned long startTime;
    unsigned long fadeStartTime;
    bool fading;
};

NoteState noteStates[128] = {0};
bool sustainPedal = false;

// Command structure for queue
struct OscCommand {
    enum Type {
        NOTE_ON,
        NOTE_OFF,
        CC,
        PITCH_BEND,
        PROGRAM_CHANGE
    };
    
    Type type;
    uint8_t note;
    uint8_t velocity;
    uint8_t controller;
    uint8_t value;
    float bendValue;
    uint8_t effectId;
};

// Network task (Core 0)
void networkTask(void *parameter) {
    Serial.println("Network task started on core " + String(xPortGetCoreID()));
    
    // Setup WiFi
    WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
    while (WiFi.status() != WL_CONNECTED) {
        delay(500);
        Serial.print(".");
    }
    Serial.println("\nWiFi connected");
    Serial.println("IP address: " + WiFi.localIP().toString());
    
    // Setup mDNS
    if (MDNS.begin("esp32-visualizer")) {
        Serial.println("mDNS responder started");
        MDNS.addService("osc", "udp", OSC_PORT);
    }
    
    // Setup OSC server
    osc_server.bind(OSC_PORT);
    
    // OSC message handlers
    osc_server.on("/noteOn", [](OscMessage& m) {
        OscCommand cmd;
        cmd.type = OscCommand::NOTE_ON;
        cmd.note = m.arg<int>(0);
        cmd.velocity = m.arg<int>(1);
        
        if (xQueueSend(commandQueue, &cmd, 0) != pdTRUE) {
            Serial.println("Failed to queue NOTE_ON command");
        }
    });
    
    osc_server.on("/noteOff", [](OscMessage& m) {
        OscCommand cmd;
        cmd.type = OscCommand::NOTE_OFF;
        cmd.note = m.arg<int>(0);
        
        if (xQueueSend(commandQueue, &cmd, 0) != pdTRUE) {
            Serial.println("Failed to queue NOTE_OFF command");
        }
    });
    
    osc_server.on("/cc", [](OscMessage& m) {
        OscCommand cmd;
        cmd.type = OscCommand::CC;
        cmd.controller = m.arg<int>(0);
        cmd.value = m.arg<int>(1);
        
        if (xQueueSend(commandQueue, &cmd, 0) != pdTRUE) {
            Serial.println("Failed to queue CC command");
        }
    });
    
    osc_server.on("/pitchBend", [](OscMessage& m) {
        OscCommand cmd;
        cmd.type = OscCommand::PITCH_BEND;
        cmd.bendValue = m.arg<float>(0);
        
        if (xQueueSend(commandQueue, &cmd, 0) != pdTRUE) {
            Serial.println("Failed to queue PITCH_BEND command");
        }
    });
    
    osc_server.on("/config/setEffect", [](OscMessage& m) {
        OscCommand cmd;
        cmd.type = OscCommand::PROGRAM_CHANGE;
        cmd.effectId = m.arg<int>(0);
        
        if (xQueueSend(commandQueue, &cmd, 0) != pdTRUE) {
            Serial.println("Failed to queue PROGRAM_CHANGE command");
        }
    });
    
    // Main network loop
    while (true) {
        osc_server.parse();
        delay(1); // Small delay to prevent watchdog issues
    }
}

// Animation task (Core 1)
void animationTask(void *parameter) {
    Serial.println("Animation task started on core " + String(xPortGetCoreID()));
    
    // Initialize FastLED
    FastLED.addLeds<LED_TYPE, LED_PIN, COLOR_ORDER>(leds, NUM_LEDS);
    FastLED.setBrightness(BRIGHTNESS);
    FastLED.clear();
    FastLED.show();
    
    unsigned long lastFrame = 0;
    const unsigned long frameInterval = 1000 / ANIMATION_FPS;
    
    while (true) {
        unsigned long currentTime = millis();
        
        // Process OSC commands
        OscCommand cmd;
        while (xQueueReceive(commandQueue, &cmd, 0) == pdTRUE) {
            processOscCommand(cmd);
        }
        
        // Update animations
        updateNoteAnimations(currentTime);
        
        // Render frame at target FPS
        if (currentTime - lastFrame >= frameInterval) {
            renderFrame();
            lastFrame = currentTime;
        }
        
        delay(1); // Small delay to prevent watchdog issues
    }
}

void processOscCommand(const OscCommand& cmd) {
    switch (cmd.type) {
        case OscCommand::NOTE_ON:
            if (cmd.note < 128) {
                noteStates[cmd.note].active = true;
                noteStates[cmd.note].velocity = cmd.velocity;
                noteStates[cmd.note].startTime = millis();
                noteStates[cmd.note].fading = false;
                Serial.printf("Note ON: %d, Velocity: %d\n", cmd.note, cmd.velocity);
            }
            break;
            
        case OscCommand::NOTE_OFF:
            if (cmd.note < 128 && noteStates[cmd.note].active) {
                if (sustainPedal) {
                    // Hold note until sustain is released
                    noteStates[cmd.note].fading = false;
                } else {
                    // Start fade out
                    noteStates[cmd.note].fading = true;
                    noteStates[cmd.note].fadeStartTime = millis();
                }
                Serial.printf("Note OFF: %d\n", cmd.note);
            }
            break;
            
        case OscCommand::CC:
            if (cmd.controller == 64) { // Sustain pedal
                sustainPedal = (cmd.value >= 64);
                if (!sustainPedal) {
                    // Release all held notes
                    for (int i = 0; i < 128; i++) {
                        if (noteStates[i].active && !noteStates[i].fading) {
                            noteStates[i].fading = true;
                            noteStates[i].fadeStartTime = millis();
                        }
                    }
                }
                Serial.printf("Sustain: %s\n", sustainPedal ? "ON" : "OFF");
            }
            break;
            
        case OscCommand::PITCH_BEND:
            // Implement pitch bend visualization
            Serial.printf("Pitch Bend: %.2f\n", cmd.bendValue);
            break;
            
        case OscCommand::PROGRAM_CHANGE:
            // Implement effect change
            Serial.printf("Program Change: %d\n", cmd.effectId);
            break;
    }
}

void updateNoteAnimations(unsigned long currentTime) {
    for (int i = 0; i < 128; i++) {
        if (noteStates[i].active) {
            if (noteStates[i].fading) {
                // Handle fade out
                unsigned long fadeTime = currentTime - noteStates[i].fadeStartTime;
                if (fadeTime > SUSTAIN_HOLD_TIME) {
                    noteStates[i].active = false;
                    noteStates[i].fading = false;
                }
            }
        }
    }
}

void renderFrame() {
    // Clear all LEDs
    FastLED.clear();
    
    // Map MIDI notes to LED positions (chromatic scale)
    for (int note = 0; note < 128; note++) {
        if (noteStates[note].active) {
            int ledIndex = note % NUM_LEDS;
            
            // Calculate color based on note and velocity
            uint8_t hue = (note * 2) % 256; // Color based on note
            uint8_t saturation = 255;
            uint8_t value = map(noteStates[note].velocity, 0, VELOCITY_MAX, 50, 255);
            
            // Apply fade effect
            if (noteStates[note].fading) {
                unsigned long fadeTime = millis() - noteStates[note].fadeStartTime;
                uint8_t fadeValue = map(fadeTime, 0, SUSTAIN_HOLD_TIME, value, 0);
                value = fadeValue;
            }
            
            // Blend with existing LED color
            CRGB newColor = CHSV(hue, saturation, value);
            leds[ledIndex] += newColor;
        }
    }
    
    // Show the frame
    FastLED.show();
}

void setup() {
    Serial.begin(115200);
    Serial.println("ESP32 Visualizer Starting...");
    
    // Create command queue
    commandQueue = xQueueCreate(32, sizeof(OscCommand));
    if (commandQueue == NULL) {
        Serial.println("Failed to create command queue");
        return;
    }
    
    // Create network task on Core 0
    xTaskCreatePinnedToCore(
        networkTask,
        "NetworkTask",
        8192,
        NULL,
        2,
        &networkTaskHandle,
        0
    );
    
    // Create animation task on Core 1
    xTaskCreatePinnedToCore(
        animationTask,
        "AnimationTask",
        4096,
        NULL,
        1,
        &animationTaskHandle,
        1
    );
    
    Serial.println("Tasks created successfully");
}

void loop() {
    // Main loop is empty - everything runs in tasks
    delay(1000);
} 