use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::evolution::{EvolutionManager, GrowthStage, Species};
use super::garden::ParadiseIsland;
use super::items::ShopItem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pet {
    pub name: String,
    pub stage: GrowthStage,
    pub species: Species,
    pub hunger: u32,       // 0 (empty) to 4 (full)
    pub happiness: u32,    // 0 to 4
    pub energy: f32,       // 0.0 to 100.0
    pub hygiene: f32,      // 0.0 to 100.0
    pub discipline: u32,   // 0 to 100
    pub poop_count: u32,   // 0 to 4
    pub is_sick: bool,
    pub is_sleeping: bool,
    pub age_days: u32,
    pub weight_g: u32,
    pub generation: u32,
    pub care_mistakes: u32,
    pub coins: u32,
    pub last_tick: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub hunger_timer: f32,
    pub happiness_timer: f32,
    pub poop_timer: f32,
    pub age_timer: f32,
    pub evolution_timer: f32,
}

impl Default for Pet {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            name: "Gotchi".to_string(),
            stage: GrowthStage::Baby,
            species: Species::Babytchi,
            hunger: 4,
            happiness: 4,
            energy: 100.0,
            hygiene: 100.0,
            discipline: 50,
            poop_count: 0,
            is_sick: false,
            is_sleeping: false,
            age_days: 0,
            weight_g: 5,
            generation: 1,
            care_mistakes: 0,
            coins: 50,
            last_tick: now,
            created_at: now,
            hunger_timer: 0.0,
            happiness_timer: 0.0,
            poop_timer: 0.0,
            age_timer: 0.0,
            evolution_timer: 0.0,
        }
    }
}

pub enum PetActionFeedback {
    Success,
    AlreadyFull,
    AlreadyClean,
    AlreadyHealthy,
    NotEnoughCoins,
    Asleep,
}

impl Pet {
    pub fn new_egg() -> Self {
        let mut pet = Self::default();
        pet.stage = GrowthStage::Egg;
        pet.species = Species::EggNormal;
        pet.hunger = 4;
        pet.happiness = 4;
        pet
    }

    pub fn tick(&mut self, dt: f32, island: &mut ParadiseIsland) -> Vec<String> {
        let mut events = Vec::new();
        self.last_tick = Utc::now();

        if self.stage == GrowthStage::Egg {
            self.evolution_timer += dt;
            if self.evolution_timer >= 10.0 {
                self.stage = GrowthStage::Baby;
                self.species = Species::Babytchi;
                self.evolution_timer = 0.0;
                events.push("egg_hatched".to_string());
            }
            return events;
        }

        if self.is_sleeping {
            self.energy = (self.energy + 5.0 * dt).min(100.0);
            return events;
        }

        // Energy decay
        self.energy = (self.energy - 0.2 * dt).max(0.0);
        if self.energy <= 5.0 {
            self.is_sleeping = true;
            events.push("fell_asleep".to_string());
        }

        // Hunger decay
        self.hunger_timer += dt;
        let hunger_interval = match self.stage {
            GrowthStage::Baby => 30.0,
            GrowthStage::Child => 60.0,
            GrowthStage::Teen => 90.0,
            _ => 120.0,
        };
        if self.hunger_timer >= hunger_interval {
            self.hunger_timer = 0.0;
            if self.hunger > 0 {
                self.hunger -= 1;
            } else {
                self.care_mistakes += 1;
                self.is_sick = true;
                events.push("hungry_alert".to_string());
            }
        }

        // Happiness decay
        self.happiness_timer += dt;
        let happiness_interval = 75.0;
        if self.happiness_timer >= happiness_interval {
            self.happiness_timer = 0.0;
            if self.happiness > 0 {
                self.happiness -= 1;
            } else {
                self.care_mistakes += 1;
            }
        }

        // Poop generation
        self.poop_timer += dt;
        let poop_interval = 140.0;
        if self.poop_timer >= poop_interval {
            self.poop_timer = 0.0;
            if self.poop_count < 4 {
                self.poop_count += 1;
                self.hygiene = (self.hygiene - 25.0).max(0.0);
                if self.poop_count >= 3 {
                    self.is_sick = true;
                    events.push("hygiene_alert".to_string());
                }
            }
        }

        // Age timer (1 day = 180 seconds for desktop experience)
        self.age_timer += dt;
        if self.age_timer >= 180.0 {
            self.age_timer = 0.0;
            self.age_days += 1;
            self.coins += 20;
            events.push("birthday".to_string());
        }

        // Evolution timer
        self.evolution_timer += dt;
        let evo_threshold = match self.stage {
            GrowthStage::Baby => 60.0,
            GrowthStage::Child => 180.0,
            GrowthStage::Teen => 300.0,
            _ => f32::MAX,
        };

        if self.evolution_timer >= evo_threshold {
            self.evolution_timer = 0.0;
            let (new_stage, new_species) = EvolutionManager::next_species(
                self.stage,
                self.species,
                self.discipline,
                self.care_mistakes,
                island.active_biome.id(),
            );
            if new_stage != self.stage || new_species != self.species {
                self.stage = new_stage;
                self.species = new_species;
                events.push("evolved".to_string());
            }
        }

        events
    }

    pub fn feed(&mut self, item: &ShopItem) -> PetActionFeedback {
        if self.is_sleeping {
            return PetActionFeedback::Asleep;
        }

        if self.hunger >= 4 && item.hunger_restore > 0 {
            return PetActionFeedback::AlreadyFull;
        }

        self.hunger = (self.hunger + item.hunger_restore).min(4);
        self.happiness = (self.happiness + item.happiness_restore).min(4);
        self.weight_g += item.weight_gain_g;
        PetActionFeedback::Success
    }

    pub fn clean_room(&mut self) -> PetActionFeedback {
        if self.poop_count == 0 {
            return PetActionFeedback::AlreadyClean;
        }
        self.poop_count = 0;
        self.hygiene = 100.0;
        PetActionFeedback::Success
    }

    pub fn heal(&mut self) -> PetActionFeedback {
        if !self.is_sick {
            return PetActionFeedback::AlreadyHealthy;
        }
        self.is_sick = false;
        PetActionFeedback::Success
    }

    pub fn train_discipline(&mut self) -> PetActionFeedback {
        if self.is_sleeping {
            return PetActionFeedback::Asleep;
        }
        self.discipline = (self.discipline + 15).min(100);
        PetActionFeedback::Success
    }

    pub fn toggle_sleep(&mut self) {
        self.is_sleeping = !self.is_sleeping;
    }
}
