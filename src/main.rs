#![no_main]
#![no_std]

use defmt_rtt as _; // global logger
use panic_halt as _;

use core::sync::atomic::{AtomicU32, Ordering};
use cortex_m_rt::entry;
use embedded_hal::spi::{Mode, Phase, Polarity};
use stm32l4xx_hal::{delay::Delay, interrupt, pac, prelude::*, spi::Spi, timer::Timer};

// Logging macros
use defmt::*;

mod wifi;

// Global timestamp counter (milliseconds since boot)
static TIMESTAMP_MS: AtomicU32 = AtomicU32::new(0);

// TIM2 interrupt handler for timestamp
#[interrupt]
fn TIM2() {
    // Clear the interrupt flag
    unsafe {
        let tim2 = &*pac::TIM2::ptr();
        tim2.sr.modify(|_, w| w.uif().clear_bit());
    }
    TIMESTAMP_MS.fetch_add(1, Ordering::Relaxed);
}

// defmt timestamp function - returns milliseconds since boot
defmt::timestamp!("{=u32:ms}", { TIMESTAMP_MS.load(Ordering::Relaxed) });

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

    // Configure TIM2 for 1ms timestamp interrupts
    let mut timer = Timer::tim2(dp.TIM2, 1000.Hz(), clocks, &mut rcc.apb1r1);
    timer.listen(stm32l4xx_hal::timer::Event::TimeOut);

    // Enable TIM2 interrupt in NVIC
    unsafe {
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::TIM2);
    }

    // Create a delay abstraction based on SysTick
    let mut delay = Delay::new(cp.SYST, clocks);
    info!("System clock and timestamp timer configured successfully");

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
    let wifi_data_ready = gpioe
        .pe1
        .into_pull_up_input(&mut gpioe.moder, &mut gpioe.pupdr);

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
        data_ready: wifi_data_ready,
    };
    let mut wifi = wifi::WifiModule::new(spi, wifi_pins);

    // Initialize WiFi module
    info!("Initializing WiFi module...");
    match wifi.init(&mut delay) {
        Ok(_) => {
            info!("WiFi module initialized successfully");

            // Test communication with WiFi module
            info!("Testing WiFi module communication...");
            match wifi.test_communication() {
                Ok(_) => {
                    info!("WiFi communication test completed");
                }
                Err(e) => {
                    warn!("WiFi communication test failed: {}", e);
                }
            }
        }
        Err(e) => {
            error!("WiFi initialization failed: {}", e);
        }
    }

    // Try to connect to WiFi network
    // Replace with your actual WiFi credentials
    let ssid = "Subway";
    let password = "5$FootLong";

    info!("Attempting to connect to WiFi network: {}", ssid);
    match wifi.connect_to_network(ssid, password, &mut delay) {
        Ok(_) => info!("WiFi connection successful"),
        Err(_) => {
            error!("Failed to connect to WiFi network...");
            loop {}
        }
    }

    // Main loop - slow blink to show system is running
    info!("Entering main loop - system operational");
    let mut loop_count = 0u32;
    loop {
        led.set_high();
        delay.delay_ms(500u16);
        led.set_low();
        delay.delay_ms(500u16);

        loop_count += 1;
        if loop_count % 5 == 0 {
            info!("System heartbeat - loop count: {}", loop_count);
        }
    }
}
