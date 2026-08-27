pub mod display;
pub mod fuses;
pub mod gpio;
pub mod snsys;
pub mod sys;
pub mod timers;
pub mod uart;
pub mod xip;

pub use display::{DisplayController, LCD_HEIGHT, LCD_WIDTH};
pub use fuses::FuseRegisters;
pub use gpio::GpioController;
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
        }
    }
}
