#![no_std]
#![no_main]

use panic_rtt_target as _;

use rtic::app;

use rtt_target::{rprintln, rtt_init_print};
use stm32l4xx_hal::prelude::*;

#[app(device = stm32l4xx_hal::pac, peripherals = true)]
const APP: () = {
    #[init]
    fn init(cx: init::Context) {
        rtt_init_print!();
        rprintln!("Firmware starting...");

        let _cp = cx.core;
        let dp = cx.device;

        // Clock configuration.
        let mut rcc = dp.RCC.constrain();
        let mut flash = dp.FLASH.constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);
        let _clocks =
            rcc.cfgr.sysclk(80.mhz()).freeze(&mut flash.acr, &mut pwr);
    }
};
