use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContinuousDistribution {
    Uniform { min: f32, max: f32 },
    Normal { mean: f32, std_dev: f32 },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkConfig {
    pub rtt: ContinuousDistribution,
    pub throughput: ContinuousDistribution,
    pub loss_rate: ContinuousDistribution,
}
