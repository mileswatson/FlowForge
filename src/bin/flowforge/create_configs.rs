use std::{fs::create_dir_all, path::Path};

use anyhow::Result;
use flowforge::{
    evaluator::EvaluationConfig,
    flow::{AlphaFairness, UtilityConfig},
    networks::remy::RemyNetworkConfig,
    quantities::seconds,
    trainers::{
        delay_multiplier::DelayMultiplierConfig, remy::RemyConfig, remyr::RemyrConfig,
        TrainerConfig,
    },
    Config,
};

pub fn create_all_configs(folder: &Path) -> Result<()> {
    create_dir_all(folder.join("eval"))?;
    create_dir_all(folder.join("network"))?;
    create_dir_all(folder.join("trainer/remy"))?;
    create_dir_all(folder.join("trainer/remyr"))?;
    create_dir_all(folder.join("trainer/delay_multiplier"))?;
    create_dir_all(folder.join("utility"))?;

    EvaluationConfig::default().save(&folder.join("eval/default.json"))?;
    EvaluationConfig {
        network_samples: 100,
        run_sim_for: seconds(60.),
    }
    .save(&folder.join("eval/short.json"))?;
    EvaluationConfig {
        network_samples: 30,
        run_sim_for: seconds(60.),
    }
    .save(&folder.join("eval/very_short.json"))?;

    RemyNetworkConfig::default().save(&folder.join("network/default.json"))?;

    TrainerConfig::Remy(RemyConfig::default()).save(&folder.join("trainer/remy/default.json"))?;
    TrainerConfig::Remyr(RemyrConfig::default())
        .save(&folder.join("trainer/remyr/default.json"))?;
    TrainerConfig::DelayMultiplier(DelayMultiplierConfig::default())
        .save(&folder.join("trainer/delay_multiplier/default.json"))?;

    UtilityConfig::AlphaFairness(AlphaFairness::MINIMISE_FIXED_LENGTH_FILE_TRANSFER)
        .save(&folder.join("utility/mflft_default.json"))?;
    UtilityConfig::AlphaFairness(AlphaFairness::PROPORTIONAL_THROUGHPUT_DELAY_FAIRNESS)
        .save(&folder.join("utility/ptdf_default.json"))?;
    Ok(())
}
