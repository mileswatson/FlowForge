use serde::{Deserialize, Serialize};

use self::{delay_multiplier::DelayMultiplierConfig, remy::RemyConfig};

pub mod delay_multiplier;
pub mod genetic;
pub mod remy;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TrainerConfig {
    Remy(RemyConfig),
    DelayMultiplier(DelayMultiplierConfig),
}
