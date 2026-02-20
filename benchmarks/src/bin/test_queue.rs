#![no_main]
#![no_std]

use defmt_rtt as _;
use panic_probe as _;

use atsamd_hal::{
    clock::GenericClockController,
    pac::{CorePeripherals, Interrupt, NVIC, Peripherals},
};

#[cortex_m_edf_rtic::app(
    device = atsamd_hal::pac,
    dispatchers = [SERCOM0_0, SERCOM0_1, SERCOM0_2],
    cpu_freq = 120_000_000,
)]
mod app {

    use super::*;

    #[shared]
    struct Shared {
        x: u32,
    }

    #[init]
    fn system_init() -> Shared {
        let mut peripherals = Peripherals::take().unwrap();
        let _core = CorePeripherals::take().unwrap();

        let _clocks = GenericClockController::with_external_32kosc(
            peripherals.gclk,
            &mut peripherals.mclk,
            &mut peripherals.osc32kctrl,
            &mut peripherals.oscctrl,
            &mut peripherals.nvmctrl,
        );

        // let timer_clock = clocks.gclk0();
        // let tc45 = &clocks.tc4_tc5(&timer_clock).unwrap();

        // // Instantiate a timer object for the TC4 timer/counter
        // let mut timer = TimerCounter::tc4_(tc45, peripherals.tc4, &mut
        // peripherals.mclk); timer.start(500.millis());
        // // timer.enable_interrupt();

        // // Instantiate a timer object for the TC5 timer/counter
        // let mut timer = TimerCounter::tc5_(tc45, peripherals.tc5, &mut
        // peripherals.mclk); timer.start(100.millis());
        // // timer.enable_interrupt();

        NVIC::pend(Interrupt::SERCOM1_0);
        NVIC::pend(Interrupt::SERCOM1_1);
        NVIC::pend(Interrupt::SERCOM1_2);

        Shared { x: 0 }
    }

    #[idle]
    pub struct MyIdleTask {
        _count: u32,
    }
    impl RticIdleTask for MyIdleTask {
        fn init() -> Self {
            Self { _count: 0 }
        }

        fn exec(&mut self) -> ! {
            loop {
                core::hint::spin_loop();
                // defmt::trace!("Idle");
            }
        }
    }

    #[task(deadline_us = 1_000_000, binds = SERCOM1_0, shared = [x])]
    pub struct Task0 {}

    impl RticTask for Task0 {
        fn init() -> Self {
            Self {}
        }

        fn exec(&mut self) {
            let mut a = 0;
            self.shared().x.lock(|x| {
                *x += 1;
                a = *x;
            });

            defmt::info!("Manual task: x = {}", a);
        }
    }

    #[task(deadline_us = 4_000_000, binds = SERCOM1_1, shared = [x])]
    pub struct ShortTask {}

    impl RticTask for ShortTask {
        fn init() -> Self {
            Self {}
        }

        fn exec(&mut self) {
            let mut a = 0;
            self.shared().x.lock(|x| {
                *x += 1;
                a = *x;
            });

            cortex_m::asm::delay(2_000_000);
            defmt::info!("Short task x = {}", a);
        }
    }

    #[task(deadline_us = 8_000_000, binds = SERCOM1_2, shared = [x])]
    pub struct LongTask {}

    impl RticTask for LongTask {
        fn init() -> Self {
            Self {}
        }

        fn exec(&mut self) {
            let mut a = 0;
            self.shared().x.lock(|x| {
                *x += 1;
                a = *x;
            });

            cortex_m::asm::delay(4_000_000);
            defmt::warn!("Long task x = {}", a);
        }
    }
}
