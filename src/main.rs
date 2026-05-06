#![no_std]
#![no_main]

use cortex_m_rt::entry;
use stm32f1xx_hal::{pac, prelude::*};
use panic_halt as _;

#[entry]
fn main() -> ! {
    // 1. 获取外设
    let dp = pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    // 2. 配置系统时钟
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();

    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .sysclk(72.MHz())
        .hclk(72.MHz())
        .pclk1(36.MHz())
        .pclk2(72.MHz())
        .freeze(&mut flash.acr);

    // 3. 初始化延时器（核心：精准延时）
    let mut delay = cp.SYST.delay(&clocks);

    // 4. 初始化 PC13
    let mut gpioc = dp.GPIOC.split();
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
    let mut i:u32 = 0;

    // 5. 循环：1秒翻转一次
    loop {
        led.toggle();
        delay.delay_us(500_000_u32); // 延时 1000ms = 1秒
        i=i+1;
    }
}