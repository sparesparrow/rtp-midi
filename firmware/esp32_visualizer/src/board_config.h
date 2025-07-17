#pragma once

// Hardware Configuration
#define LED_PIN       16
#define NUM_LEDS      23
#define LED_TYPE      WS2812B
#define COLOR_ORDER   GRB
#define BRIGHTNESS    150
#define VOLTS         5
#define MAX_AMPS      1500 // 23 LEDs * 60mA/LED = 1380mA

// Network Configuration
#define WIFI_SSID     "YourWiFiSSID"
#define WIFI_PASSWORD "YourWiFiPassword"
#define OSC_PORT      8000

// Built-in LED for status
#define BUILTIN_LED   2

// Animation Configuration
#define ANIMATION_FPS 60
#define FADE_SPEED    5
#define SUSTAIN_HOLD_TIME 2000 // ms

// MIDI Configuration
#define MIDI_CHANNEL  1
#define VELOCITY_MAX  127 