//! WiFi module driver for ISM43362 using eS-WiFi protocol
//!
//! This module provides a driver for the ISM43362 WiFi module found on the
//! STM32L475 Discovery board. It implements the eS-WiFi command protocol
//! using 16-bit SPI transfers as specified in the ISM43362 datasheet.
//!
//! The implementation is based on the es-wifi-driver reference implementation
//! and provides basic WiFi connectivity functionality.

use cortex_m::asm::nop;
use defmt::{debug, error, info, warn};
use embedded_hal::blocking::{delay::DelayMs, spi::Transfer};
use heapless::String;

use stm32l4xx_hal::{
    gpio::{gpiob::*, gpioc::*, gpioe::*, Alternate, Input, Output, PullUp, PushPull},
    pac::SPI3,
    spi::Spi,
};

// WiFi module pins on STM32L475 Discovery board
// SPI3_SCK  -> PC10 (connected to ISM43362 SPI_CLK)
// SPI3_MOSI -> PC12 (connected to ISM43362 SPI_MOSI)
// SPI3_MISO -> PC11 (connected to ISM43362 SPI_MISO)
// WiFi_CS   -> PE0  (Chip Select)
// WiFi_RST  -> PE8  (Reset)
// WiFi_WKUP -> PB13 (Wake up)

/// SPI peripheral type for WiFi communication
pub type WifiSpi = Spi<
    SPI3,
    (
        PC10<Alternate<PushPull, 6>>, // SCK
        PC11<Alternate<PushPull, 6>>, // MISO
        PC12<Alternate<PushPull, 6>>, // MOSI
    ),
>;

/// GPIO pins used for WiFi module control
pub struct WifiPins {
    /// Chip Select pin (PE0)
    pub cs: PE0<Output<PushPull>>,
    /// Reset pin (PE8)
    pub reset: PE8<Output<PushPull>>,
    /// Wake-up pin (PB13)
    pub wakeup: PB13<Output<PushPull>>,
    /// Data Ready pin (PE1) - indicates when module is ready for communication
    pub data_ready: PE1<Input<PullUp>>,
}

/// WiFi connection states
#[derive(Debug, Clone, Copy)]
pub enum WifiState {
    /// Module is disconnected from any network
    Disconnected = 0,
    /// Module is connected to a WiFi network
    Connected = 1,
}

/// Main WiFi module driver structure
///
/// This structure encapsulates the SPI peripheral, GPIO pins, and state
/// needed to communicate with the ISM43362 WiFi module using the eS-WiFi protocol.
pub struct WifiModule {
    /// SPI peripheral for communication
    pub spi: WifiSpi,
    /// GPIO pins for module control
    pub pins: WifiPins,
    /// Current connection state
    state: WifiState,
}

impl WifiModule {
    pub fn new(spi: WifiSpi, pins: WifiPins) -> Self {
        Self {
            spi,
            pins,
            state: WifiState::Disconnected,
        }
    }

    pub fn init(&mut self, delay: &mut impl DelayMs<u32>) -> Result<(), &'static str> {
        info!("Starting WiFi module reset sequence...");

        // Reset the WiFi module (as per es-wifi-driver timing)
        self.pins.reset.set_low();
        delay.delay_ms(50);
        self.pins.reset.set_high();
        delay.delay_ms(50);
        info!("WiFi module reset completed");

        // Wake up the module (as per es-wifi-driver timing)
        self.pins.wakeup.set_high();
        delay.delay_ms(50);
        info!("WiFi module wake-up signal sent");

        // Fetch initial cursor as required by ISM43362 spec
        info!("Fetching initial cursor...");
        match self.fetch_initial_cursor(delay) {
            Ok(cursor) => info!("Successfully fetched initial cursor: '{}'", cursor.as_str()),
            Err(e) => warn!("Failed to fetch initial cursor: {}", e),
        }

        // Disable verbosity as per es-wifi-driver
        info!("Disabling verbosity...");
        let _response = self.send_at_command("MT=1\r")?;

        // Test basic communication using eS-WiFi commands
        info!("Testing basic eS-WiFi communication...");
        let version_response = self.send_at_command("MR\r")?; // Get module version
        info!("Module version info: {}", version_response.as_str());

        info!("WiFi module initialization completed successfully");
        Ok(())
    }

    pub fn check_data_ready_pin(&self) -> bool {
        // According to ISM43362 spec: CMD/DATA READY pin HIGH = data ready
        self.pins.data_ready.is_high()
    }

    /// Fetch initial cursor after power-up/reset
    pub fn fetch_initial_cursor(
        &mut self,
        delay: &mut impl DelayMs<u32>,
    ) -> Result<String<64>, &'static str> {
        info!("Fetching initial cursor...");

        // Wait for CMD/DATA READY pin to go HIGH (data ready)
        let mut timeout = 0;
        while !self.check_data_ready_pin() && timeout < 1000 {
            delay.delay_ms(10);
            timeout += 1;
        }

        if timeout >= 1000 {
            return Err("Timeout waiting for initial data ready");
        }

        info!("Data ready pin is HIGH, fetching cursor...");

        // Select the WiFi module
        self.pins.cs.set_low();
        delay.delay_ms(1);

        let mut cursor = String::<64>::new();

        // Clock out 0x0A (Line Feed) until CMD/DATA READY pin goes LOW
        // Using 8-bit transfers but following 16-bit protocol (send MSB first, then LSB)
        while self.check_data_ready_pin() {
            // Send 16-bit word as two 8-bit transfers: MSB first, then LSB
            let mut tx_msb = [0x0A]; // MSB: Line Feed
            let rx_msb = self
                .spi
                .transfer(&mut tx_msb)
                .map_err(|_| "SPI transfer failed")?;

            let mut tx_lsb = [0x00]; // LSB: 0x00
            let rx_lsb = self
                .spi
                .transfer(&mut tx_lsb)
                .map_err(|_| "SPI transfer failed")?;

            // Store received data from both bytes
            for &received_byte in &[rx_msb[0], rx_lsb[0]] {
                if received_byte >= 32 && received_byte <= 126 {
                    cursor
                        .push(received_byte as char)
                        .map_err(|_| "Cursor too long")?;
                }
            }
        }

        // Deselect the WiFi module
        self.pins.cs.set_high();
        delay.delay_ms(1);

        info!("Received cursor: '{}'", cursor.as_str());
        Ok(cursor)
    }

    pub fn test_communication(&mut self) -> Result<(), &'static str> {
        info!("Testing WiFi module communication...");

        // Check initial data ready pin state
        let data_ready_initial = self.check_data_ready_pin();
        info!(
            "Initial data ready pin state: {}",
            if data_ready_initial {
                "READY"
            } else {
                "NOT READY"
            }
        );

        // Send a simple eS-WiFi command to test communication
        info!("Sending test eS-WiFi command...");

        // Get MAC address
        let response = self.send_at_command("Z5\r")?;
        info!("MAC address: {}", response.as_str());

        Ok(())
    }

    /// Send command using 16-bit SPI transfers as per ISM43362 spec
    fn send_command_16bit(&mut self, command: &str) -> Result<(), &'static str> {
        info!("Sending 16-bit command: {}", command.trim());

        // Select the WiFi module (as per es-wifi-driver timing)
        self.pins.cs.set_low();

        // Send command bytes using 16-bit protocol as per es-wifi-driver
        let cmd_bytes: heapless::Vec<u8, 256> = command.bytes().collect();
        for chunk in cmd_bytes.chunks(2) {
            let mut xfer: [u8; 2] = [0; 2];
            xfer[1] = chunk[0]; // LSB gets first byte
            if chunk.len() == 2 {
                xfer[0] = chunk[1]; // MSB gets second byte
            } else {
                xfer[0] = 0x0A; // MSB gets 0x0A if odd length
            }

            self.spi
                .transfer(&mut xfer)
                .map_err(|_| "SPI transfer failed")?;
        }

        // Deselect the WiFi module (minimal hold time as per es-wifi-driver)
        self.pins.cs.set_high();
        // No delay needed here - es-wifi-driver uses only 15 microseconds

        // Check data ready pin state after sending command
        debug!(
            "Data ready pin after command: {}",
            if self.check_data_ready_pin() {
                "HIGH"
            } else {
                "LOW"
            }
        );

        Ok(())
    }

    /// Read response using 16-bit SPI transfers as per ISM43362 spec
    fn read_response_16bit(&mut self) -> Result<String<256>, &'static str> {
        // Wait for data ready signal
        debug!("Waiting for data ready signal...");
        while !self.check_data_ready_pin() {
            nop();
        }

        info!("Data ready for response, reading...");

        // Select the WiFi module
        self.pins.cs.set_low();
        let mut response = String::<256>::new();
        // Clock out 0x0A (Line Feed) until CMD/DATA READY pin goes LOW
        // Using 16-bit protocol as per es-wifi-driver
        while self.check_data_ready_pin() {
            let mut xfer: [u8; 2] = [0x0A, 0x0A]; // Send 0x0A in both bytes
            self.spi
                .transfer(&mut xfer)
                .map_err(|_| "SPI transfer failed")?;

            // Store received data, checking for NAK (0x15)
            const NAK: u8 = 0x15;

            // Process in reverse order as per es-wifi-driver (16 -> 2*8 bits)
            if xfer[1] != NAK {
                response
                    .push(xfer[1] as char)
                    .map_err(|_| "Response too long")?;
            }
            if xfer[0] != NAK {
                response
                    .push(xfer[0] as char)
                    .map_err(|_| "Response too long")?;
            }
        }

        // Validation
        let mut lines = response.lines();
        let _empty_line = lines
            .next()
            .ok_or("No starting empty line in command response")?;
        let first_line = lines.next().ok_or("No response data")?;
        let reply = lines.next().ok_or("No response reply code")?;

        if reply != "OK" {
            warn!("Failed command: {}", reply);
            return Err("Command failed");
        }

        let data = String::<256>::try_from(first_line)
            .map_err(|_| "Could not represent data as string")?;

        // Deselect the WiFi module
        self.pins.cs.set_high();
        Ok(data)
    }

    pub fn connect_to_network(
        &mut self,
        ssid: &str,
        password: &str,
        delay: &mut impl DelayMs<u32>,
    ) -> Result<(), &'static str> {
        info!("Starting WiFi connection process...");

        // Disconnect from any existing network using eS-WiFi command
        info!("Disconnecting from any existing network...");
        let _response = self.send_at_command("CD\r")?; // Disconnect command

        // Set security mode to WPA2 (CB=2) as per es-wifi-driver
        info!("Setting security mode to WPA2...");
        let _response = self.send_at_command("CB=2\r")?; // WPA2 security mode

        // Set SSID using eS-WiFi command
        info!("Setting SSID: {}", ssid);
        let mut ssid_cmd: String<128> = String::new();
        ssid_cmd.push_str("C1=").map_err(|_| "Command too long")?;
        ssid_cmd.push_str(ssid).map_err(|_| "SSID too long")?;
        ssid_cmd.push_str("\r").map_err(|_| "Command too long")?;
        let _response = self.send_at_command(ssid_cmd.as_str())?;

        // Set password using eS-WiFi command
        info!("Setting password...");
        let mut pwd_cmd: String<128> = String::new();
        pwd_cmd.push_str("C2=").map_err(|_| "Command too long")?;
        pwd_cmd
            .push_str(password)
            .map_err(|_| "Password too long")?;
        pwd_cmd.push_str("\r").map_err(|_| "Command too long")?;
        let _response = self.send_at_command(pwd_cmd.as_str())?;

        // Set encryption type (C3=4 for WPA2) as per es-wifi-driver
        info!("Setting encryption type...");
        let _response = self.send_at_command("C3=4\r")?; // WPA2 encryption

        // Connect to WiFi network using eS-WiFi command
        info!("Connecting to WiFi network: {}", ssid);
        let _response = self.send_at_command("C0\r")?; // Connect command

        // Check connection status in a loop with ~10 second timeout
        info!("Waiting for WiFi connection...");
        let mut connection_attempts = 0;
        const MAX_CONNECTION_ATTEMPTS: u32 = 20; // 20 attempts * 500ms = 10 seconds

        loop {
            delay.delay_ms(500); // Wait 500ms between checks
            connection_attempts += 1;

            match self.send_at_command("C?\r") {
                Ok(response) => {
                    // Parse the response to check if connection was successful
                    // Look for IP address pattern (xxx.xxx.xxx.xxx) which indicates successful connection
                    if response.contains("192.168.")
                        || response.contains("10.")
                        || response.contains("172.")
                    {
                        info!("WiFi connection successful! Status: {}", response.as_str());
                        self.state = WifiState::Connected;
                        break;
                    } else if response.contains("Failed") {
                        warn!("WiFi connection failed: {}", response.as_str());
                        return Err("WiFi connection failed");
                    } else if response.len() > 0 {
                        debug!(
                            "Connection attempt {}/{}: {}",
                            connection_attempts,
                            MAX_CONNECTION_ATTEMPTS,
                            response.as_str()
                        );
                    } else {
                        debug!(
                            "Connection attempt {}/{}: (empty response)",
                            connection_attempts, MAX_CONNECTION_ATTEMPTS
                        );
                    }
                }
                Err(e) => {
                    debug!(
                        "Failed to check connection status (attempt {}): {}",
                        connection_attempts, e
                    );
                }
            }

            if connection_attempts >= MAX_CONNECTION_ATTEMPTS {
                warn!(
                    "WiFi connection timeout after {} attempts",
                    MAX_CONNECTION_ATTEMPTS
                );
                return Err("WiFi connection timeout");
            }
        }

        info!("WiFi connection process completed");
        Ok(())
    }

    fn send_at_command(&mut self, command: &str) -> Result<String<256>, &'static str> {
        debug!("Sending AT command: {}", command.trim());

        // Send the command using 16-bit protocol
        self.send_command_16bit(command)?;

        // Read the response using 16-bit protocol
        match self.read_response_16bit() {
            Ok(response) => {
                info!("Response: {}", response.as_str());
                Ok(response)
            }
            Err(e) => {
                warn!("Failed to read response: {}", e);
                // Return an empty response on error
                Ok(String::new())
            }
        }
    }
}
