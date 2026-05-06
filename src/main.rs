#![no_std]
#![no_main]

use cortex_m_rt::entry;
use stm32f4xx_hal::{
    pac,
    prelude::*,
    timer::Timer,
};
use panic_halt as _;

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let rcc = dp.RCC.constrain();

    // 先用默认内部时钟，不使用外部晶振
    let clocks = rcc.cfgr.freeze();

    let mut delay = Timer::syst(cp.SYST, &clocks).delay();

    let gpiob = dp.GPIOB.split();

    // 对应 C 里的 GPIO_MODE_OUTPUT_PP
    let mut led = gpiob.pb2.into_push_pull_output();

    // 对应 HAL_GPIO_WritePin(..., GPIO_PIN_RESET)
    led.set_low();

    loop {
        led.set_low();
        delay.delay_ms(500_u32);

        led.set_high();
        delay.delay_ms(500_u32);
    }
}
