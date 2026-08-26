pub enum SecretReward {
    Coins(u32),
    UnlockOcean,
    UnlockSky,
    GoldenApples(u32),
    FullHeal,
}

pub struct SecretCodeManager;

impl SecretCodeManager {
    pub fn redeem(code: &str) -> Option<SecretReward> {
        let cleaned = code.trim().to_uppercase();
        match cleaned.as_str() {
            "TAMA-PARA-2026" | "PARADISE2026" => Some(SecretReward::Coins(150)),
            "OCEAN-DEEP-BLUE" | "OCEAN777" => Some(SecretReward::UnlockOcean),
            "SKY-HIGH-STAR" | "SKY999" => Some(SecretReward::UnlockSky),
            "GOLDEN-FRUIT-99" | "APPLEMAX" => Some(SecretReward::GoldenApples(5)),
            "HEALTH-BOOST" => Some(SecretReward::FullHeal),
            _ => None,
        }
    }
}
