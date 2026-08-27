pub mod adc;
pub mod crc;
pub mod display;
pub mod flashctl;
pub mod fuses;
pub mod gpio;
pub mod gpio_port;
pub mod snsys;
pub mod sys;
pub mod timers;
pub mod uart;
pub mod xip;

pub use adc::SarAdc;
pub use crc::{Calcul, ChecksumUnit};
pub use display::{DisplayController, LCD_HEIGHT, LCD_WIDTH};
pub use flashctl::{FlashController, Transfer};
pub use fuses::FuseRegisters;
pub use gpio::GpioController;
pub use gpio_port::GpioPort;
pub use snsys::SnSysRegisters;
pub use sys::SysRegisters;
pub use timers::Timers;
pub use uart::UartController;
pub use xip::XipController;

pub struct Peripherals {
    pub sys: SysRegisters,
    pub fuses: FuseRegisters,
    pub snsys: SnSysRegisters,
    pub display: DisplayController,
    pub gpio: GpioController,
    pub uart: UartController,
    pub timers: Timers,
    pub xip: XipController,
    /// Les deux convertisseurs, 0x4000A000 et 0x4000B000.
    pub adc: [SarAdc; 2],
    pub flashctl: FlashController,
    pub crc: ChecksumUnit,
    /// Port 2, celui dont le firmware lit les broches 0 et 1.
    pub port0: GpioPort,
    pub port1: GpioPort,
    pub port2: GpioPort,
}

impl Default for Peripherals {
    fn default() -> Self {
        Self {
            sys: SysRegisters::default(),
            fuses: FuseRegisters::default(),
            snsys: SnSysRegisters::default(),
            display: DisplayController::default(),
            gpio: GpioController::default(),
            uart: UartController::default(),
            timers: Timers::default(),
            xip: XipController::default(),
            adc: Default::default(),
            flashctl: FlashController::default(),
            crc: ChecksumUnit::default(),
            port0: GpioPort::default(),
            port1: GpioPort::port1(),
            port2: GpioPort::default(),
        }
    }
}
