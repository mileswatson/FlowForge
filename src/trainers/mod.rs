use serde::{Deserialize, Serialize};

use self::remy::RemyConfig;

pub mod remy;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TrainerConfig {
    Remy(RemyConfig),
}
