#![no_std]
#![no_main]

use panic_rtt_target as _;

use rtic::app;

use stm32l4xx_hal::{
    gpio::{Alternate, Floating, Input, Output, PushPull, AF7, PA10, PA5, PA9},
    pac::USART1,
    prelude::*,
    serial::{self, Config, Serial},
};

use ercp_basic::{
    ack, adapter::SerialAdapter, command::nack_reason, nack, Command,
    ErcpBasic, Router,
};
use rtt_target::{rprintln, rtt_init_print};

/// The board LED.
type Led = PA5<Output<PushPull>>;

/// The UART we use for ERCP.
type Uart = Serial<
    USART1,
    (
        PA9<Alternate<AF7, Input<Floating>>>,
        PA10<Alternate<AF7, Input<Floating>>>,
    ),
>;

/// Resources that are driveable via ERCP.
pub struct DriveableResources {
    led: Led,
    counter: u8,
}

/// Our custom ERCP router.
pub struct CustomRouter {
    buffer: [u8; TX_MAX_LEN],
}

impl Default for CustomRouter {
    fn default() -> Self {
        Self {
            buffer: [0; TX_MAX_LEN],
        }
    }
}

impl Router<RX_MAX_LEN> for CustomRouter {
    type Context = DriveableResources;

    fn route(
        &mut self,
        command: Command,
        cx: &mut Self::Context,
    ) -> Option<Command> {
        match command.command() {
            // Override the route method to add our routes.
            LED_ON => self.led_on(&mut cx.led),
            LED_OFF => self.led_off(&mut cx.led),
            COUNTER_GET => self.counter_get(cx.counter),
            COUNTER_SET => self.counter_set(command, &mut cx.counter),
            COUNTER_INC => self.counter_inc(&mut cx.counter),
            COUNTER_DEC => self.counter_dec(&mut cx.counter),

            // Always end with default routes.
            _ => self.default_routes(command),
        }
    }

    // Customise the firmware version & description.

    fn firmware_version(&self) -> &str {
        concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"))
    }

    fn description(&self) -> &str {
        "Example ERCP firmware"
    }
}

impl CustomRouter {
    /// Creates a new router.
    pub fn new() -> Self {
        Self::default()
    }

    // Command handlers are here.

    fn led_on(&mut self, led: &mut Led) -> Option<Command> {
        led.set_high().ok();
        Some(ack!())
    }

    fn led_off(&mut self, led: &mut Led) -> Option<Command> {
        led.set_low().ok();
        Some(ack!())
    }

    fn counter_get(&mut self, counter: u8) -> Option<Command> {
        self.buffer[0] = counter;
        let reply =
            Command::new(COUNTER_GET_REPLY, &self.buffer[0..1]).unwrap();
        Some(reply)
    }

    fn counter_set(
        &mut self,
        command: Command,
        counter: &mut u8,
    ) -> Option<Command> {
        if command.length() == 1 {
            *counter = command.value()[0];
            Some(ack!())
        } else {
            Some(nack!(nack_reason::INVALID_ARGUMENTS))
        }
    }

    fn counter_inc(&mut self, counter: &mut u8) -> Option<Command> {
        match counter.checked_add(1) {
            Some(value) => {
                *counter = value;
                Some(ack!())
            }

            None => Some(nack!(OUT_OF_BOUNDS)),
        }
    }

    fn counter_dec(&mut self, counter: &mut u8) -> Option<Command> {
        match counter.checked_sub(1) {
            Some(value) => {
                *counter = value;
                Some(ack!())
            }

            None => Some(nack!(OUT_OF_BOUNDS)),
        }
    }
}

// Rx & Tx buffer sizes.
const RX_MAX_LEN: usize = 255;
const TX_MAX_LEN: usize = 255;

// Commands.
const LED_ON: u8 = 0x20;
const LED_OFF: u8 = 0x21;
const COUNTER_GET: u8 = 0x30;
const COUNTER_GET_REPLY: u8 = 0x31;
const COUNTER_SET: u8 = 0x32;
const COUNTER_INC: u8 = 0x33;
const COUNTER_DEC: u8 = 0x34;

const OUT_OF_BOUNDS: u8 = 0xFF;

#[app(device = stm32l4xx_hal::pac, peripherals = true)]
const APP: () = {
    struct Resources {
        ercp: ErcpBasic<SerialAdapter<Uart>, CustomRouter, RX_MAX_LEN>,
        driveable_resources: DriveableResources,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        rtt_init_print!();
        rprintln!("Firmware starting...");

        let _cp = cx.core;
        let dp = cx.device;

        // Clock configuration.
        let mut rcc = dp.RCC.constrain();
        let mut flash = dp.FLASH.constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);
        let clocks = rcc.cfgr.freeze(&mut flash.acr, &mut pwr);

        let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);

        // LED configuration.
        let led = gpioa
            .pa5
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);

        // Serial port configuration.
        let tx_pin = gpioa.pa9.into_af7(&mut gpioa.moder, &mut gpioa.afrh);
        let rx_pin = gpioa.pa10.into_af7(&mut gpioa.moder, &mut gpioa.afrh);
        let mut serial = Serial::usart1(
            dp.USART1,
            (tx_pin, rx_pin),
            Config::default().baudrate(115_200.bps()),
            clocks,
            &mut rcc.apb2,
        );

        // Listen RX events.
        serial.listen(serial::Event::Rxne);

        // ERCP configuration.
        let adapter = SerialAdapter::new(serial);
        let router = CustomRouter::new();
        let mut ercp = ErcpBasic::new(adapter, router);

        rprintln!("Firmware initialised!");
        ercp.log("Firmware initialised!").ok();

        init::LateResources {
            ercp,
            driveable_resources: DriveableResources { led, counter: 0 },
        }
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            continue;
        }
    }

    #[task(binds = USART1, resources = [ercp], spawn = [ercp_process])]
    fn usart1(cx: usart1::Context) {
        let ercp = cx.resources.ercp;

        ercp.handle_data();

        if ercp.complete_frame_received() {
            cx.spawn.ercp_process().ok();
        }
    }

    #[task(resources = [ercp, driveable_resources])]
    fn ercp_process(cx: ercp_process::Context) {
        cx.resources.ercp.process(cx.resources.driveable_resources);
    }

    extern "C" {
        fn TIM2();
    }
};
