#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

#[rtic::app(device = stm32l4xx_hal::pac, dispatchers = [TIM2])]
mod app {
    use stm32l4xx_hal::{
        gpio::{Alternate, PushPull, PA10, PA9},
        pac::USART1,
        prelude::*,
        serial::{self, Config, Serial},
    };

    use ercp_basic::{adapter::SerialAdapter, ErcpBasic};

    use ercp_basic_example::{CustomRouter, DriveableResources};

    /// The UART we use for ERCP.
    type Uart = Serial<
        USART1,
        (PA9<Alternate<PushPull, 7>>, PA10<Alternate<PushPull, 7>>),
    >;

    #[shared]
    struct SharedResources {
        ercp: ErcpBasic<SerialAdapter<Uart>, CustomRouter>,
    }

    #[local]
    struct LocalResources {
        driveable_resources: DriveableResources,
    }

    #[init]
    fn init(
        cx: init::Context,
    ) -> (SharedResources, LocalResources, init::Monotonics) {
        defmt::info!("Firmware starting...");

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
        let tx_pin = gpioa.pa9.into_alternate(
            &mut gpioa.moder,
            &mut gpioa.otyper,
            &mut gpioa.afrh,
        );
        let rx_pin = gpioa.pa10.into_alternate(
            &mut gpioa.moder,
            &mut gpioa.otyper,
            &mut gpioa.afrh,
        );
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

        defmt::info!("Firmware initialised!");
        ercp.log("Firmware initialised!").ok();

        (
            SharedResources { ercp },
            LocalResources {
                driveable_resources: DriveableResources::new(led),
            },
            init::Monotonics(),
        )
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            continue;
        }
    }

    #[task(binds = USART1, shared = [ercp])]
    fn usart1(mut cx: usart1::Context) {
        defmt::trace!("Receiving data on UART");

        cx.shared.ercp.lock(|ercp| {
            ercp.handle_data().ok();

            if ercp.complete_frame_received() {
                defmt::trace!("Complete frame received!");
                ercp_process::spawn().ok();
            }
        });
    }

    #[task(shared = [ercp], local = [driveable_resources])]
    fn ercp_process(mut cx: ercp_process::Context) {
        defmt::debug!("Processing an ERCP frame...");

        let driveable_resources = cx.local.driveable_resources;
        cx.shared
            .ercp
            .lock(|ercp| ercp.process(driveable_resources).ok());
    }
}
