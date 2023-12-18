use std::path::Path;

use anyhow::Result;
use flowforge::{
    flow::{AlphaFairness, UtilityConfig},
    network::config::NetworkConfig,
    trainers::{delay_multiplier::DelayMultiplierConfig, remy::RemyConfig, TrainerConfig},
    Config,
};

use crate::{ConfigCommand, TrainerConfigCommand, UtilityConfigCommand};

pub(super) fn create_config(config_type: &ConfigCommand, output: &Path) -> Result<()> {
    match config_type {
        ConfigCommand::Network => NetworkConfig::default().save(output),
        ConfigCommand::Trainer(config_type) => {
            let trainer_config = match config_type {
                TrainerConfigCommand::Remy => TrainerConfig::Remy(RemyConfig::default()),
                TrainerConfigCommand::DelayMultiplier => {
                    TrainerConfig::DelayMultiplier(DelayMultiplierConfig::default())
                }
            };

            trainer_config.save(output)
        }
        ConfigCommand::Utility(config_type) => UtilityConfig::AlphaFairness(match config_type {
            UtilityConfigCommand::ProportionalThroughputDelayFairness => {
                AlphaFairness::PROPORTIONAL_THROUGHPUT_DELAY_FAIRNESS
            }
            UtilityConfigCommand::MinimiseFixedLengthFileTransfer => {
                AlphaFairness::MINIMISE_FIXED_LENGTH_FILE_TRANSFER
            }
        })
        .save(output),
    }
}
