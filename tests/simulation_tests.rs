use tamagotchi_paradise_rs::core::evolution::{EvolutionManager, GrowthStage, Species};
use tamagotchi_paradise_rs::core::garden::ParadiseIsland;
use tamagotchi_paradise_rs::core::items::get_default_catalog;
use tamagotchi_paradise_rs::core::minigames::{BerryCatchState, ParadiseWheelState};
use tamagotchi_paradise_rs::core::pet::{Pet, PetActionFeedback};
use tamagotchi_paradise_rs::core::secrets::{SecretCodeManager, SecretReward};
use tamagotchi_paradise_rs::hw_bridge::flash_map::FlashInspector;
use tamagotchi_paradise_rs::hw_bridge::uart_pcom::UartBridge;
use tamagotchi_paradise_rs::i18n::{I18n, Language};

#[test]
fn test_pet_lifecycle_and_feeding() {
    let mut pet = Pet::default();

    // Start with default
    assert_eq!(pet.stage, GrowthStage::Baby);
    assert_eq!(pet.species, Species::Babytchi);

    // Feed meal
    let catalog = get_default_catalog();
    let burger = catalog.iter().find(|i| i.id == "food_burger").unwrap();
    pet.hunger = 2;
    let res = pet.feed(burger);
    assert!(matches!(res, PetActionFeedback::Success));
    assert_eq!(pet.hunger, 4);

    // Poop and cleaning
    pet.poop_count = 2;
    let clean_res = pet.clean_room();
    assert!(matches!(clean_res, PetActionFeedback::Success));
    assert_eq!(pet.poop_count, 0);
    assert_eq!(pet.hygiene, 100.0);

    // Evolution check
    let (stage, species) = EvolutionManager::next_species(
        GrowthStage::Child,
        Species::Tamatchi,
        80,
        0,
        "garden",
    );
    assert_eq!(stage, GrowthStage::Teen);
    assert_eq!(species, Species::YoungMametchi);
}

#[test]
fn test_paradise_island_biomes() {
    let mut island = ParadiseIsland::default();
    assert!(island.garden_unlocked);
    assert!(!island.ocean_unlocked);

    // Water plants
    let (watered, level) = island.water_plants();
    assert!(watered);
    assert_eq!(level, 2);

    // Micro clean
    island.micro_cell_health = 50.0;
    island.cleanse_micro_cells();
    assert_eq!(island.micro_cell_health, 75.0);
}

#[test]
fn test_secret_codes() {
    let reward1 = SecretCodeManager::redeem("TAMA-PARA-2026");
    assert!(matches!(reward1, Some(SecretReward::Coins(150))));

    let reward2 = SecretCodeManager::redeem("OCEAN-DEEP-BLUE");
    assert!(matches!(reward2, Some(SecretReward::UnlockOcean)));

    let invalid = SecretCodeManager::redeem("UNKNOWN-CODE-000");
    assert!(invalid.is_none());
}

#[test]
fn test_minigames_logic() {
    let mut berry_game = BerryCatchState::new();
    berry_game.move_left(0.1);
    assert!(berry_game.basket_x < 0.5);

    let mut wheel_game = ParadiseWheelState::new();
    assert!(wheel_game.is_spinning);
    let won = wheel_game.stop();
    assert!(!wheel_game.is_spinning);
    assert!(won || !won);
}

#[test]
fn test_flash_inspector_layout() {
    let inspector = FlashInspector::new();
    assert_eq!(inspector.sections.len(), 6);
    assert_eq!(inspector.sections[0].name, "Firmware Header");
    assert_eq!(inspector.sections[2].offset_start, 0x011000);
    assert_eq!(inspector.header_magic, "SONIXDEV");
}

#[test]
fn test_uart_packet_encoding() {
    let packet = UartBridge::encode_packet(0x42, &[0x01, 0x02, 0x03]);
    assert_eq!(&packet[0..4], &[0xAB, 0x5D, 0xEB, 0xEF]);
    assert_eq!(packet[4], 0x42);
}

#[test]
fn test_i18n_multilingual() {
    let mut i18n = I18n::new(Language::Fr);
    assert_eq!(i18n.t("app_title"), "Tamagotchi Paradise Desktop");
    assert_eq!(i18n.t("btn_feed"), "Nourrir");

    i18n.set_language(Language::En);
    assert_eq!(i18n.t("btn_feed"), "Feed");
    assert_eq!(
        i18n.t_args("dialog_feed_success", &[("name", "Gotchi")]),
        "Gotchi enjoyed the meal."
    );
}
