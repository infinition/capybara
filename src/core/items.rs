use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemCategory {
    Meal,
    Snack,
    Toy,
    Medicine,
    BiomeKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopItem {
    pub id: &'static str,
    pub name_key: &'static str,
    pub category: ItemCategory,
    pub price: u32,
    pub hunger_restore: u32,
    pub happiness_restore: u32,
    pub weight_gain_g: u32,
}

pub fn get_default_catalog() -> Vec<ShopItem> {
    vec![
        ShopItem {
            id: "meal_rice",
            name_key: "food_meal",
            category: ItemCategory::Meal,
            price: 5,
            hunger_restore: 1,
            happiness_restore: 0,
            weight_gain_g: 1,
        },
        ShopItem {
            id: "food_burger",
            name_key: "food_burger",
            category: ItemCategory::Meal,
            price: 15,
            hunger_restore: 2,
            happiness_restore: 1,
            weight_gain_g: 3,
        },
        ShopItem {
            id: "food_sushi",
            name_key: "food_sushi",
            category: ItemCategory::Meal,
            price: 25,
            hunger_restore: 3,
            happiness_restore: 2,
            weight_gain_g: 2,
        },
        ShopItem {
            id: "snack_candy",
            name_key: "food_candy",
            category: ItemCategory::Snack,
            price: 5,
            hunger_restore: 0,
            happiness_restore: 1,
            weight_gain_g: 2,
        },
        ShopItem {
            id: "food_cake",
            name_key: "food_cake",
            category: ItemCategory::Snack,
            price: 20,
            hunger_restore: 1,
            happiness_restore: 2,
            weight_gain_g: 4,
        },
        ShopItem {
            id: "food_apple",
            name_key: "food_apple",
            category: ItemCategory::Snack,
            price: 10,
            hunger_restore: 1,
            happiness_restore: 1,
            weight_gain_g: 1,
        },
    ]
}
