use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BiomeType {
    #[default]
    Garden,
    Ocean,
    Sky,
}

impl BiomeType {
    pub fn id(&self) -> &'static str {
        match self {
            BiomeType::Garden => "garden",
            BiomeType::Ocean => "ocean",
            BiomeType::Sky => "sky",
        }
    }

    pub fn title_key(&self) -> &'static str {
        match self {
            BiomeType::Garden => "biome_garden",
            BiomeType::Ocean => "biome_ocean",
            BiomeType::Sky => "biome_sky",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParadiseIsland {
    pub active_biome: BiomeType,
    pub garden_unlocked: bool,
    pub ocean_unlocked: bool,
    pub sky_unlocked: bool,
    pub flora_growth_level: u32,
    pub harvested_fruits: u32,
    pub micro_cell_health: f32, // 0.0 to 100.0 for microscopic zoom inspection
}

impl Default for ParadiseIsland {
    fn default() -> Self {
        Self {
            active_biome: BiomeType::Garden,
            garden_unlocked: true,
            ocean_unlocked: false,
            sky_unlocked: false,
            flora_growth_level: 1,
            harvested_fruits: 0,
            micro_cell_health: 100.0,
        }
    }
}

impl ParadiseIsland {
    pub fn switch_biome(&mut self, biome: BiomeType) -> bool {
        let is_unlocked = match biome {
            BiomeType::Garden => self.garden_unlocked,
            BiomeType::Ocean => self.ocean_unlocked,
            BiomeType::Sky => self.sky_unlocked,
        };

        if is_unlocked {
            self.active_biome = biome;
            true
        } else {
            false
        }
    }

    pub fn water_plants(&mut self) -> (bool, u32) {
        if self.flora_growth_level < 5 {
            self.flora_growth_level += 1;
            (true, self.flora_growth_level)
        } else {
            self.harvested_fruits += 1;
            (false, self.harvested_fruits)
        }
    }

    pub fn cleanse_micro_cells(&mut self) {
        self.micro_cell_health = (self.micro_cell_health + 25.0).min(100.0);
    }
}
