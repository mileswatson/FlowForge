use std::{fs::create_dir_all, path::Path};

use anyhow::Result;
use flowforge::{
    flow::{AlphaFairness, UtilityConfig},
    network::config::NetworkConfig,
    trainers::{delay_multiplier::DelayMultiplierConfig, remy::RemyConfig, TrainerConfig},
    Config,
};

pub fn create_all_configs(folder: &Path) -> Result<()> {
    create_dir_all(folder.join("network"))?;
    create_dir_all(folder.join("trainer/remy"))?;
    create_dir_all(folder.join("trainer/delay_multiplier"))?;
    create_dir_all(folder.join("utility"))?;

    NetworkConfig::default().save(&folder.join("network/default.json"))?;

    TrainerConfig::Remy(RemyConfig::default()).save(&folder.join("trainer/remy/default.json"))?;
    TrainerConfig::DelayMultiplier(DelayMultiplierConfig::default())
        .save(&folder.join("trainer/delay_multiplier/default.json"))?;

    UtilityConfig::AlphaFairness(AlphaFairness::minimized_fixed_length_file_transfer())
        .save(&folder.join("utility/mflft_default.json"))?;
    UtilityConfig::AlphaFairness(AlphaFairness::proportional_throughput_delay_fairness())
        .save(&folder.join("utility/ptdf_default.json"))?;
    Ok(())
}
