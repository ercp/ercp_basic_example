#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

#[rtic::app(device = stm32l4xx_hal::pac, dispatchers = [TIM2])]
mod app {
    use systick_monotonic::Systick;

    use stm32l4xx_hal::{
        gpio::{Alternate, PushPull, PA2, PA3},
        pac::USART2,
        prelude::*,
        serial::{self, Config, Serial},
    };

    use ercp_basic::{adapter::SerialAdapter, ErcpBasic};

    use ercp_basic_example::{CustomRouter, DriveableResources};

    /// The UART we use for ERCP.
    type Uart = Serial<
        USART2,
        (PA2<Alternate<PushPull, 7>>, PA3<Alternate<PushPull, 7>>),
    >;

    const MONO_FREQ: u32 = 100;

    #[monotonic(binds = SysTick, default = true)]
    type Monotonic = Systick<MONO_FREQ>;

    /// A timer for ERCP timeouts based on the monotonic.
    pub struct MonotonicTimer;

    impl ercp_basic::Timer for MonotonicTimer {
        type Instant = systick_monotonic::fugit::Instant<u64, 1, MONO_FREQ>;
        type Duration = systick_monotonic::fugit::Duration<u64, 1, MONO_FREQ>;

        fn now(&mut self) -> Self::Instant {
            monotonics::now()
        }
    }

    #[shared]
    struct SharedResources {
        ercp: ErcpBasic<SerialAdapter<Uart>, MonotonicTimer, CustomRouter>,
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

        let cp = cx.core;
        let dp = cx.device;

        let monotonic = Systick::new(cp.SYST, 80_000_000);

        // Clock configuration.
        let mut rcc = dp.RCC.constrain();
        let mut flash = dp.FLASH.constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);
        let clocks = rcc.cfgr.sysclk(80.MHz()).freeze(&mut flash.acr, &mut pwr);

        let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);

        // LED configuration.
        let led = gpioa
            .pa5
            .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);

        // Serial port configuration.
        let tx_pin = gpioa.pa2.into_alternate(
            &mut gpioa.moder,
            &mut gpioa.otyper,
            &mut gpioa.afrl,
        );
        let rx_pin = gpioa.pa3.into_alternate(
            &mut gpioa.moder,
            &mut gpioa.otyper,
            &mut gpioa.afrl,
        );
        let mut serial = Serial::usart2(
            dp.USART2,
            (tx_pin, rx_pin),
            Config::default().baudrate(115_200.bps()),
            clocks,
            &mut rcc.apb1r1,
        );

        // Listen RX events.
        serial.listen(serial::Event::Rxne);

        // ERCP configuration.
        let adapter = SerialAdapter::new(serial);
        let router = CustomRouter::new();
        let mut ercp = ErcpBasic::new(adapter, MonotonicTimer, router);

        defmt::info!("Firmware initialised!");
        ercp.log("Firmware initialised!").ok();

        (
            SharedResources { ercp },
            LocalResources {
                driveable_resources: DriveableResources::new(led),
            },
            init::Monotonics(monotonic),
        )
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            continue;
        }
    }

    #[task(binds = USART2, shared = [ercp])]
    fn usart2(mut cx: usart2::Context) {
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
