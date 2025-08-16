# STM32L475 Discovery WiFi Blink

This project demonstrates basic WiFi connectivity on the STM32L475 Discovery board using Rust and the embedded ISM43362 WiFi module.

## Features

- LED blinking to indicate system status
- WiFi module initialization via SPI
- WiFi network connection using AT commands
- HTTP GET request functionality
- Visual feedback through LED patterns

## Hardware

- STM32L475 Discovery Board (B-L475E-IOT01A)
- Built-in ISM43362-M3G-L44 WiFi module
- LED on PA5 (LD1)

## WiFi Configuration

Before flashing, update the WiFi credentials in `src/main.rs`:

```rust
let ssid = "YourWiFiSSID";
let password = "YourWiFiPassword";
```
