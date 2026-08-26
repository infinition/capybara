pub mod display;
pub mod gpio;
pub mod sys;
pub mod timers;
pub mod uart;

pub use display::{DisplayController, LCD_HEIGHT, LCD_WIDTH};
pub use gpio::GpioController;
pub use sys::SysRegisters;
pub use timers::Timers;
pub use uart::UartController;

pub struct Peripherals {
    pub sys: SysRegisters,
    pub display: DisplayController,
    pub gpio: GpioController,
    pub uart: UartController,
    pub timers: Timers,
}

impl Default for Peripherals {
    fn default() -> Self {
        Self {
            sys: SysRegisters::default(),
            display: DisplayController::default(),
            gpio: GpioController::default(),
            uart: UartController::default(),
            timers: Timers::default(),
        }
    }
}
