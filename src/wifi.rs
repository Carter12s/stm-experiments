use defmt::{debug, error, info, warn};
use embedded_hal::blocking::{delay::DelayMs, spi::Transfer};
use heapless::{String, Vec};
use nb::block;
use stm32l4xx_hal::{
    gpio::{gpiob::*, gpioc::*, gpioe::*, Alternate, Output, PushPull},
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

pub type WifiSpi = Spi<
    SPI3,
    (
        PC10<Alternate<PushPull, 6>>, // SCK
        PC11<Alternate<PushPull, 6>>, // MISO
        PC12<Alternate<PushPull, 6>>, // MOSI
    ),
>;

pub struct WifiPins {
    pub cs: PE0<Output<PushPull>>,
    pub reset: PE8<Output<PushPull>>,
    pub wakeup: PB13<Output<PushPull>>,
}

// ISM43362 SPI frame structure
const SPI_FRAME_START: u8 = 0x15;
const SPI_FRAME_END: u8 = 0x16;

// WiFi connection states
#[derive(Debug, Clone, Copy)]
pub enum WifiState {
    Disconnected = 0,
    Connected = 1,
    GotIp = 2,
    Connecting = 3,
}

pub struct WifiModule {
    pub spi: WifiSpi,
    pub pins: WifiPins,
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

        // Reset the WiFi module
        self.pins.reset.set_low();
        delay.delay_ms(10);
        self.pins.reset.set_high();
        delay.delay_ms(500);
        info!("WiFi module reset completed");

        // Wake up the module
        self.pins.wakeup.set_high();
        delay.delay_ms(100);
        info!("WiFi module wake-up signal sent");

        // Test basic communication
        info!("Testing basic AT communication...");
        self.send_at_command("AT\r\n", delay)?;
        delay.delay_ms(100);

        // Reset to factory defaults
        info!("Resetting WiFi module to factory defaults...");
        self.send_at_command("AT&F\r\n", delay)?;
        delay.delay_ms(1000);

        // Set WiFi mode to station
        info!("Setting WiFi mode to station...");
        self.send_at_command("AT+CWMODE=1\r\n", delay)?;
        delay.delay_ms(500);

        info!("WiFi module initialization completed successfully");
        Ok(())
    }

    pub fn connect_to_network(
        &mut self,
        ssid: &str,
        password: &str,
        delay: &mut impl DelayMs<u32>,
    ) -> Result<(), &'static str> {
        info!("Starting WiFi connection process...");

        // Disconnect from any existing network
        info!("Disconnecting from any existing network...");
        self.send_at_command("AT+CWQAP\r\n", delay)?;
        delay.delay_ms(1000);

        // Create the join command
        info!("Creating join command for SSID: {}", ssid);
        let mut join_cmd: String<128> = String::new();
        join_cmd
            .push_str("AT+CWJAP=\"")
            .map_err(|_| "Command too long")?;
        join_cmd.push_str(ssid).map_err(|_| "SSID too long")?;
        join_cmd.push_str("\",\"").map_err(|_| "Command too long")?;
        join_cmd
            .push_str(password)
            .map_err(|_| "Password too long")?;
        join_cmd
            .push_str("\"\r\n")
            .map_err(|_| "Command too long")?;

        // Send join command
        info!("Sending WiFi join command...");
        self.send_at_command(&join_cmd, delay)?;

        // Wait for connection (this can take several seconds)
        info!("Waiting for WiFi connection (up to 10 seconds)...");
        delay.delay_ms(10000);

        // Check connection status
        info!("Checking WiFi connection status...");
        self.send_at_command("AT+CWJAP?\r\n", delay)?;
        delay.delay_ms(500);

        self.state = WifiState::Connected;
        info!("WiFi connection process completed");
        Ok(())
    }

    pub fn get_status(&mut self) -> WifiState {
        self.state
    }

    fn send_at_command(
        &mut self,
        command: &str,
        delay: &mut impl DelayMs<u32>,
    ) -> Result<(), &'static str> {
        debug!("Sending AT command: {}", command.trim());

        // Select the WiFi module
        self.pins.cs.set_low();
        delay.delay_ms(1);

        // Send SPI frame start
        let mut tx_buf = [SPI_FRAME_START];
        let mut rx_buf = [0u8; 1];
        self.spi
            .transfer(&mut tx_buf)
            .map_err(|_| "SPI transfer failed")?;

        // Send command length (2 bytes, little endian)
        let cmd_len = command.len() as u16;
        let len_bytes = cmd_len.to_le_bytes();
        let mut tx_len = [len_bytes[0], len_bytes[1]];
        self.spi
            .transfer(&mut tx_len)
            .map_err(|_| "SPI transfer failed")?;

        // Send command data
        for byte in command.bytes() {
            let mut tx_data = [byte];
            self.spi
                .transfer(&mut tx_data)
                .map_err(|_| "SPI transfer failed")?;
        }

        // Send frame end
        let mut tx_end = [SPI_FRAME_END];
        self.spi
            .transfer(&mut tx_end)
            .map_err(|_| "SPI transfer failed")?;

        // Deselect the WiFi module
        self.pins.cs.set_high();
        delay.delay_ms(1);

        Ok(())
    }

    pub fn send_http_get(
        &mut self,
        host: &str,
        path: &str,
        delay: &mut impl DelayMs<u32>,
    ) -> Result<(), &'static str> {
        // Create TCP connection
        let mut connect_cmd: String<128> = String::new();
        connect_cmd
            .push_str("AT+CIPSTART=\"TCP\",\"")
            .map_err(|_| "Command too long")?;
        connect_cmd.push_str(host).map_err(|_| "Host too long")?;
        connect_cmd
            .push_str("\",80\r\n")
            .map_err(|_| "Command too long")?;

        self.send_at_command(&connect_cmd, delay)?;
        delay.delay_ms(2000);

        // Create HTTP GET request
        let mut http_request: String<256> = String::new();
        http_request
            .push_str("GET ")
            .map_err(|_| "Request too long")?;
        http_request.push_str(path).map_err(|_| "Path too long")?;
        http_request
            .push_str(" HTTP/1.1\r\nHost: ")
            .map_err(|_| "Request too long")?;
        http_request.push_str(host).map_err(|_| "Host too long")?;
        http_request
            .push_str("\r\nConnection: close\r\n\r\n")
            .map_err(|_| "Request too long")?;

        // Send data length
        let mut send_cmd: String<64> = String::new();
        send_cmd
            .push_str("AT+CIPSEND=")
            .map_err(|_| "Command too long")?;

        // Convert length to string
        let len_str = http_request.len();
        let mut len_buffer = [0u8; 10];
        let mut len_pos = 0;
        let mut temp_len = len_str;

        if temp_len == 0 {
            len_buffer[0] = b'0';
            len_pos = 1;
        } else {
            while temp_len > 0 {
                len_buffer[len_pos] = (temp_len % 10) as u8 + b'0';
                temp_len /= 10;
                len_pos += 1;
            }
            // Reverse the digits
            for i in 0..len_pos / 2 {
                len_buffer.swap(i, len_pos - 1 - i);
            }
        }

        for i in 0..len_pos {
            send_cmd
                .push(len_buffer[i] as char)
                .map_err(|_| "Command too long")?;
        }
        send_cmd.push_str("\r\n").map_err(|_| "Command too long")?;

        self.send_at_command(&send_cmd, delay)?;
        delay.delay_ms(100);

        // Send HTTP request
        self.send_at_command(&http_request, delay)?;
        delay.delay_ms(2000);

        // Close connection
        self.send_at_command("AT+CIPCLOSE\r\n", delay)?;
        delay.delay_ms(500);

        Ok(())
    }
}
