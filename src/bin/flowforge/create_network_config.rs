use std::{fs::File, path::Path};

use anyhow::Result;
use flowforge::network::config::{ContinuousDistribution, NetworkConfig};

pub fn create_network_config(output: &Path) -> Result<()> {
    let output = File::create(output)?;

    let config = NetworkConfig {
        rtt: ContinuousDistribution::Normal {
            mean: 5e-3,
            std_dev: 1e-3,
        },
        throughput: ContinuousDistribution::Uniform { min: 12., max: 18. },
        loss_rate: ContinuousDistribution::Normal {
            mean: 0.1,
            std_dev: 0.01,
        },
    };

    serde_json::to_writer_pretty(&output, &config)?;
    Ok(())
}
