use std::path::Path;

use anyhow::Result;
use flowforge::{
    network::config::NetworkConfig,
    trainers::{delay_multiplier::DelayMultiplierConfig, remy::RemyConfig, TrainerConfig},
    Config,
};

use crate::{ConfigCommand, TrainerConfigCommand};

pub(super) fn create_config(config_type: &ConfigCommand, output: &Path) -> Result<()> {
    let config_type = match config_type {
        ConfigCommand::Network => return NetworkConfig::default().save(output),
        ConfigCommand::Trainer(x) => x,
    };

    let trainer_config = match config_type {
        TrainerConfigCommand::Remy => TrainerConfig::Remy(RemyConfig::default()),
        TrainerConfigCommand::DelayMultiplier => {
            TrainerConfig::DelayMultiplier(DelayMultiplierConfig::default())
        }
    };

    trainer_config.save(output)
}
