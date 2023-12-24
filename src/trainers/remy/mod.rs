use std::convert::Into;

use anyhow::Result;
use protobuf::Message;
use serde::{Deserialize, Serialize};

use crate::{
    flow::UtilityFunction, network::config::NetworkConfig, rand::Rng, time::Float, Dna,
    ProgressHandler, Trainer,
};

pub mod rule_tree;

use self::{
    autogen::remy_dna::WhiskerTree,
    rule_tree::{Action, Point, RuleTree},
};

#[allow(clippy::all, clippy::pedantic, clippy::nursery)]
mod autogen {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RemyConfig {
    rounds: usize,
    epochs_per_round: usize,
    run_sim_for: Float,
    networks_per_iter: usize,
    min_action: Action,
    max_action: Action,
    initial_action_change: Action,
    action_change_multiplier: Float,
    default_action: Action,
}

impl Default for RemyConfig {
    fn default() -> Self {
        Self {
            rounds: 100,
            epochs_per_round: 5,
            run_sim_for: 120.,
            networks_per_iter: 1000,
            min_action: Action {
                window_multiplier: 0.,
                window_increment: 0,
                intersend_ms: 0.25,
            },
            max_action: Action {
                window_multiplier: 1.,
                window_increment: 256,
                intersend_ms: 3.,
            },
            initial_action_change: Action {
                window_multiplier: 0.01,
                window_increment: 1,
                intersend_ms: 0.05,
            },
            action_change_multiplier: 4.,
            default_action: Action {
                window_multiplier: 1.,
                window_increment: 1,
                intersend_ms: 0.01,
            },
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct RemyDna {
    tree: RuleTree,
}

impl RemyDna {
    #[must_use]
    pub fn action<const COUNT: bool>(&self, point: &Point) -> &Action {
        self.tree
            .action::<COUNT>(point)
            .unwrap_or_else(|| panic!("Point {point:?} to be within the valid range"))
    }

    #[must_use]
    pub fn default(dna: &RemyConfig) -> Self {
        RemyDna {
            tree: RuleTree::default(dna),
        }
    }
}

impl Dna for RemyDna {
    const NAME: &'static str = "remy";
    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(WhiskerTree::from(RuleTree::new_with_same_rules(&self.tree)).write_to_bytes()?)
    }

    fn deserialize(buf: &[u8]) -> Result<RemyDna> {
        Ok(RemyDna {
            tree: WhiskerTree::parse_from_bytes(buf)?.into(),
        })
    }
}

pub struct RemyTrainer {
    config: RemyConfig,
}

impl Trainer<RemyDna> for RemyTrainer {
    type Config = RemyConfig;

    fn new(config: &RemyConfig) -> RemyTrainer {
        RemyTrainer {
            config: config.clone(),
        }
    }

    fn train<H: ProgressHandler<RemyDna>>(
        &self,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> RemyDna {
        let result = RemyDna::default(&self.config);
        progress_handler.update_progress(1., Some(&result));
        result
    }
}
