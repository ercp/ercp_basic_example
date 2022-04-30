#![no_std]

use stm32l4xx_hal::{
    gpio::{Output, PushPull, PA5},
    prelude::*,
};

use ercp_basic::{ack, command::nack_reason, nack, Command, Router};

/// The board LED.
type Led = PA5<Output<PushPull>>;

/// Resources that are driveable via ERCP.
pub struct DriveableResources {
    led: Led,
    counter: u8,
}

/// Our custom ERCP router.
pub struct CustomRouter {
    buffer: [u8; TX_MAX_LEN],
}

impl DriveableResources {
    pub fn new(led: Led) -> Self {
        Self { led, counter: 0 }
    }
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
        match command.code() {
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

    // TODO: Use a macro instead to generate this.
    fn firmware_version(&self) -> &str {
        concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"))
    }

    // TODO: Use a macro instead to generate this.
    fn description(&self) -> &str {
        env!("CARGO_PKG_DESCRIPTION")
    }
}

impl CustomRouter {
    /// Creates a new router.
    pub fn new() -> Self {
        Self::default()
    }

    // Command handlers are here.

    fn led_on(&mut self, led: &mut Led) -> Option<Command> {
        defmt::info!("Led on");
        led.set_high().ok();
        Some(ack!())
    }

    fn led_off(&mut self, led: &mut Led) -> Option<Command> {
        defmt::info!("Led off");
        led.set_low().ok();
        Some(ack!())
    }

    fn counter_get(&mut self, counter: u8) -> Option<Command> {
        defmt::info!("Counter = {}", counter);
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
            defmt::info!("Setting the counter to {}", command.value()[0]);
            *counter = command.value()[0];
            Some(ack!())
        } else {
            defmt::warn!("Invalid arguments");
            Some(nack!(nack_reason::INVALID_ARGUMENTS))
        }
    }

    fn counter_inc(&mut self, counter: &mut u8) -> Option<Command> {
        match counter.checked_add(1) {
            Some(value) => {
                defmt::info!("Increasing the counter to {}", value);
                *counter = value;
                Some(ack!())
            }

            None => {
                defmt::warn!("Cannot increase the counter above 255");
                Some(nack!(OUT_OF_BOUNDS))
            }
        }
    }

    fn counter_dec(&mut self, counter: &mut u8) -> Option<Command> {
        match counter.checked_sub(1) {
            Some(value) => {
                defmt::info!("Decreasing the counter to {}", value);
                *counter = value;
                Some(ack!())
            }

            None => {
                defmt::warn!("Cannot decrease the counter below 0");
                Some(nack!(OUT_OF_BOUNDS))
            }
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
