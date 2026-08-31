//! Tests d'integration : ce que l'emulateur expose au reste du logiciel.

use capybara::hw_bridge::flash_map::FlashInspector;
use capybara::hw_bridge::uart_terminal::UartBridge;
use capybara::i18n::{I18n, Language};

#[test]
fn test_flash_inspector_layout() {
    let inspector = FlashInspector::new();
    assert_eq!(inspector.sections.len(), 6);
    assert_eq!(inspector.sections[0].name, "Firmware Header");
    assert_eq!(inspector.sections[2].offset_start, 0x011000);
    assert_eq!(inspector.header_magic, "SONIXDEV");
}

#[test]
fn test_uart_host_bridge_sync() {
    let mut bridge = UartBridge::new("COM10", 460800);
    let mut uart = capybara::emulator::peripherals::UartController::new();

    // Envoi hote vers console
    bridge.host_write(&[0x11, 0x22, 0x33]);
    bridge.sync(&mut uart);
    uart.tick(6_300, 96_000_000);

    assert_eq!(uart.rx_fifo.len(), 3);
    assert_eq!(uart.read_reg(0x00), 0x11);
    assert_eq!(uart.read_reg(0x00), 0x22);
    assert_eq!(uart.read_reg(0x00), 0x33);

    // Envoi console vers hote
    uart.write_reg(0x00, 0xAA);
    uart.write_reg(0x00, 0xBB);
    uart.tick(4_200, 96_000_000);
    bridge.sync(&mut uart);

    assert_eq!(bridge.host_read(), vec![0xAA, 0xBB]);
}

#[test]
fn test_i18n_multilingual() {
    let mut i18n = I18n::new(Language::Fr);
    assert_eq!(i18n.t("app_title"), "Capybara");
    assert_eq!(i18n.t("btn_feed"), "Nourrir");

    i18n.set_language(Language::En);
    assert_eq!(i18n.t("btn_feed"), "Feed");
    assert_eq!(
        i18n.t_args("dialog_feed_success", &[("name", "Gotchi")]),
        "Gotchi enjoyed the meal."
    );
}
