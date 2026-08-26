use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GrowthStage {
    #[default]
    Egg,
    Baby,
    Child,
    Teen,
    Adult,
    Senior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Species {
    #[default]
    EggNormal,
    Babytchi,
    Marutchi,
    Tamatchi,
    YoungMametchi,
    YoungMemetchi,
    YoungKuchipatchi,
    Mametchi,
    Memetchi,
    Kuchipatchi,
    Floragitchi,
    Coralgotchi,
    Skygotchi,
}

impl Species {
    pub fn name(&self) -> &'static str {
        match self {
            Species::EggNormal => "Egg",
            Species::Babytchi => "Babytchi",
            Species::Marutchi => "Marutchi",
            Species::Tamatchi => "Tamatchi",
            Species::YoungMametchi => "Young Mametchi",
            Species::YoungMemetchi => "Young Memetchi",
            Species::YoungKuchipatchi => "Young Kuchipatchi",
            Species::Mametchi => "Mametchi",
            Species::Memetchi => "Memetchi",
            Species::Kuchipatchi => "Kuchipatchi",
            Species::Floragitchi => "Floragitchi",
            Species::Coralgotchi => "Coralgotchi",
            Species::Skygotchi => "Skygotchi",
        }
    }
}

pub struct EvolutionManager;

impl EvolutionManager {
    pub fn next_species(
        stage: GrowthStage,
        current: Species,
        discipline: u32,
        care_mistakes: u32,
        unlocked_paradise_biome: &str,
    ) -> (GrowthStage, Species) {
        match stage {
            GrowthStage::Egg => (GrowthStage::Baby, Species::Babytchi),
            GrowthStage::Baby => {
                if discipline >= 50 {
                    (GrowthStage::Child, Species::Tamatchi)
                } else {
                    (GrowthStage::Child, Species::Marutchi)
                }
            }
            GrowthStage::Child => {
                if discipline >= 70 && care_mistakes <= 1 {
                    (GrowthStage::Teen, Species::YoungMametchi)
                } else if discipline >= 40 {
                    (GrowthStage::Teen, Species::YoungMemetchi)
                } else {
                    (GrowthStage::Teen, Species::YoungKuchipatchi)
                }
            }
            GrowthStage::Teen => {
                if unlocked_paradise_biome == "ocean" {
                    (GrowthStage::Adult, Species::Coralgotchi)
                } else if unlocked_paradise_biome == "sky" {
                    (GrowthStage::Adult, Species::Skygotchi)
                } else if unlocked_paradise_biome == "garden" && discipline >= 85 {
                    (GrowthStage::Adult, Species::Floragitchi)
                } else {
                    match current {
                        Species::YoungMametchi => {
                            if care_mistakes <= 2 {
                                (GrowthStage::Adult, Species::Mametchi)
                            } else {
                                (GrowthStage::Adult, Species::Memetchi)
                            }
                        }
                        Species::YoungMemetchi => (GrowthStage::Adult, Species::Memetchi),
                        _ => (GrowthStage::Adult, Species::Kuchipatchi),
                    }
                }
            }
            GrowthStage::Adult => (GrowthStage::Adult, current),
            GrowthStage::Senior => (GrowthStage::Senior, current),
        }
    }
}
