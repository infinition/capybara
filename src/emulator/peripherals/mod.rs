pub mod adc;
pub mod crc;
pub mod display;
pub mod flashctl;
pub mod fuses;
pub mod gpio;
pub mod adc_pile;
pub mod tic;
pub mod dma;
pub mod gpio_port;
pub mod pmu;
pub mod snsys;
pub mod spi;
pub mod sys;
pub mod timers;
pub mod uart;
pub mod usb;
pub mod xip;

pub use adc::SarAdc;
pub use crc::{Calcul, ChecksumUnit};
pub use display::{DisplayController, LCD_HEIGHT, LCD_WIDTH};
pub use flashctl::{FlashController, Transfer};
pub use fuses::FuseRegisters;
pub use gpio::GpioController;
pub use adc_pile::AdcPile;
pub use tic::TicSysteme;
pub use dma::DmaController;
pub use gpio_port::GpioPort;
pub use pmu::PmuController;
pub use snsys::SnSysRegisters;
pub use spi::SpiController;
pub use sys::SysRegisters;
pub use timers::Timers;
pub use uart::UartController;
pub use usb::UsbController;
pub use xip::XipController;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Peripherals {
    pub sys: SysRegisters,
    pub fuses: FuseRegisters,
    pub snsys: SnSysRegisters,
    pub display: DisplayController,
    pub gpio: GpioController,
    pub uart: UartController,
    pub timers: Timers,
    pub xip: XipController,
    pub adc: [SarAdc; 2],
    pub flashctl: FlashController,
    pub crc: ChecksumUnit,
    pub adc_pile: AdcPile,
    #[serde(default)]
    pub tic: TicSysteme,
    pub dma: DmaController,
    pub port0: GpioPort,
    pub port1: GpioPort,
    pub port2: GpioPort,
    #[serde(default)]
    pub spi0: SpiController,
    #[serde(default)]
    pub pmu: PmuController,
    #[serde(default)]
    pub usb: UsbController,
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
            adc_pile: AdcPile::default(),
            tic: TicSysteme::default(),
            dma: DmaController::default(),
            port0: GpioPort::default(),
            port1: GpioPort::port1(),
            port2: GpioPort::default(),
            spi0: SpiController::default(),
            pmu: PmuController::default(),
            usb: UsbController::default(),
        }
    }
}
