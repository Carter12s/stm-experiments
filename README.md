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

## LED Status Indicators

The LED provides visual feedback for different operations:

- **WiFi Init Success**: Short blink (200ms on/off)
- **WiFi Init Failed**: 10 rapid blinks (100ms on/off)
- **WiFi Connected**: 2 medium blinks (300ms on/off)
- **WiFi Connection Failed**: 20 very rapid blinks (50ms on/off)
- **HTTP Request Success**: 3 quick blinks (200ms on/off)
- **HTTP Request Failed**: Long blink (2 seconds on)
- **Normal Operation**: Slow blink (2 seconds on/off)

## Dependencies

To build this WiFi-enabled application you'll need:

- The `cargo generate` subcommand. [Installation
  instructions](https://github.com/cargo-generate/cargo-generate#installation).
``` console
$ cargo install cargo-generate
```

- Flash and run/debug tools:
``` console
$ cargo install probe-rs --features cli
```

- `rust-std` components (pre-compiled `core` crate) for the ARM Cortex-M
  targets. Run:
  
``` console
$ rustup target add thumbv6m-none-eabi thumbv7m-none-eabi thumbv7em-none-eabi thumbv7em-none-eabihf
```

## Instantiate the template.

1. Run and enter project name
``` console
$ cargo generate --git https://github.com/burrbull/stm32-template/
 Project Name: app
```

2. Specify **chip product name** and answer on several other guide questions.

3. Your program is ready to compile:
``` console
$ cargo build --release
```

## Flash and run/debug

You can flash your firmware using one of those tools:

- `cargo flash --release` — just flash
- `cargo run --release` — flash and run using `probe-rs run` runner or `probe-run` runner (deprecated) which you can set in `.cargo/config.toml`
- `cargo embed --release` — multifunctional tool for flash and debug

You also can debug your firmware on device from VS Code with [probe-rs](https://probe.rs/docs/tools/vscode/) extention or with `probe-rs gdb` command.
You will need SVD specification for your chip for this. You can load patched SVD files [here](https://stm32-rs.github.io/stm32-rs/).

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## Code of Conduct

Contribution to this crate is organized under the terms of the [Rust Code of
Conduct][CoC], the maintainer of this crate, promises
to intervene to uphold that code of conduct.

[CoC]: https://www.rust-lang.org/policies/code-of-conduct
