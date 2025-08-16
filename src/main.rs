#![no_main]
#![no_std]

use defmt_rtt as _; // global logger
use panic_halt as _;

use cortex_m_rt::entry;
use embedded_hal::spi::{Mode, Phase, Polarity};
use stm32l4xx_hal::{delay::Delay, pac, prelude::*, spi::Spi};

// Logging macros
use defmt::*;

mod wifi;

// defmt timestamp function
defmt::timestamp!("{=u64}", {
    // For now, return a simple counter or use a timer
    // In a real application, you'd use a proper timer
    static mut COUNTER: u64 = 0;
    unsafe {
        COUNTER += 1;
        COUNTER
    }
});

#[entry]
fn main() -> ! {
    info!("STM32L475 WiFi Application Starting...");
    info!("defmt RTT logging initialized");

    // Get access to the device specific peripherals
    let dp = pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    info!("Peripherals initialized");

    // Configure the system clock
    info!("Configuring system clock...");
    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();
    let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);
    let clocks = rcc.cfgr.freeze(&mut flash.acr, &mut pwr);

    // Create a delay abstraction based on SysTick
    let mut delay = Delay::new(cp.SYST, clocks);
    info!("System clock configured successfully");

    // Configure GPIO ports
    info!("Configuring GPIO ports...");
    let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);
    let mut gpiob = dp.GPIOB.split(&mut rcc.ahb2);
    let mut gpioc = dp.GPIOC.split(&mut rcc.ahb2);
    let mut gpioe = dp.GPIOE.split(&mut rcc.ahb2);

    // Configure PA5 as output (LD1 on STM32L475 Discovery)
    let mut led = gpioa
        .pa5
        .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
    info!("LED configured on PA5");

    // Configure WiFi SPI pins (AF6 for SPI3)
    let sck =
        gpioc
            .pc10
            .into_alternate_push_pull(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrh);
    let miso =
        gpioc
            .pc11
            .into_alternate_push_pull(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrh);
    let mosi =
        gpioc
            .pc12
            .into_alternate_push_pull(&mut gpioc.moder, &mut gpioc.otyper, &mut gpioc.afrh);

    // Configure WiFi control pins
    let wifi_cs = gpioe
        .pe0
        .into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);
    let wifi_reset = gpioe
        .pe8
        .into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);
    let wifi_wakeup = gpiob
        .pb13
        .into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);

    // Configure SPI3 for WiFi module
    let spi = Spi::spi3(
        dp.SPI3,
        (sck, miso, mosi),
        Mode {
            polarity: Polarity::IdleLow,
            phase: Phase::CaptureOnFirstTransition,
        },
        1.MHz(),
        clocks,
        &mut rcc.apb1r1,
    );

    // Create WiFi module
    let wifi_pins = wifi::WifiPins {
        cs: wifi_cs,
        reset: wifi_reset,
        wakeup: wifi_wakeup,
    };
    let mut wifi = wifi::WifiModule::new(spi, wifi_pins);

    // Initialize WiFi module
    info!("Initializing WiFi module...");
    match wifi.init(&mut delay) {
        Ok(_) => {
            info!("WiFi module initialized successfully");
            // WiFi initialized successfully - short blink
            led.set_high();
            delay.delay_ms(200u16);
            led.set_low();
            delay.delay_ms(200u16);
        }
        Err(e) => {
            error!("WiFi initialization failed: {}", e);
            // WiFi initialization failed - rapid blink
            for _ in 0..10 {
                led.set_high();
                delay.delay_ms(100u16);
                led.set_low();
                delay.delay_ms(100u16);
            }
        }
    }

    // Try to connect to WiFi network
    // Replace with your actual WiFi credentials
    let ssid = "Subway";
    let password = "5$FootLong";

    info!("Attempting to connect to WiFi network: {}", ssid);
    match wifi.connect_to_network(ssid, password, &mut delay) {
        Ok(_) => {
            info!("WiFi connected successfully to: {}", ssid);
            // WiFi connected successfully - double blink
            for _ in 0..2 {
                led.set_high();
                delay.delay_ms(300u16);
                led.set_low();
                delay.delay_ms(300u16);
            }

            // Try to send an HTTP GET request
            info!("Sending HTTP GET request to httpbin.org...");
            match wifi.send_http_get("httpbin.org", "/get", &mut delay) {
                Ok(_) => {
                    info!("HTTP request completed successfully");
                    // HTTP request successful - triple blink
                    for _ in 0..3 {
                        led.set_high();
                        delay.delay_ms(200u16);
                        led.set_low();
                        delay.delay_ms(200u16);
                    }
                }
                Err(e) => {
                    error!("HTTP request failed: {}", e);
                    // HTTP request failed - long blink
                    led.set_high();
                    delay.delay_ms(2000u16);
                    led.set_low();
                }
            }
        }
        Err(e) => {
            error!("WiFi connection failed: {}", e);
            // WiFi connection failed - very rapid blink
            for _ in 0..20 {
                led.set_high();
                delay.delay_ms(50u16);
                led.set_low();
                delay.delay_ms(50u16);
            }
        }
    }

    // Main loop - slow blink to show system is running
    info!("Entering main loop - system operational");
    let mut loop_count = 0u32;
    loop {
        led.set_high();
        delay.delay_ms(2000u16);
        led.set_low();
        delay.delay_ms(2000u16);

        loop_count += 1;
        if loop_count % 10 == 0 {
            info!("System heartbeat - loop count: {}", loop_count);
        }
    }
}
