use std::convert::Into;

use anyhow::Result;
use protobuf::Message;
use serde::{Deserialize, Serialize};

use crate::{
    evaluator::EvaluationConfig, flow::UtilityFunction, network::config::NetworkConfig, rand::Rng,
    Dna, ProgressHandler, Trainer,
};

pub mod rule_tree;

use self::{
    autogen::remy_dna::WhiskerTree,
    rule_tree::{Action, Point, RuleOverride, RuleTree},
};

#[allow(clippy::all, clippy::pedantic, clippy::nursery)]
mod autogen {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RemyConfig {
    rule_splits: usize,
    optimization_rounds_per_split: usize,
    min_action: Action,
    max_action: Action,
    initial_action_change: Action,
    action_change_multiplier: i32,
    default_action: Action,
    evaluation_config: EvaluationConfig,
}

impl Default for RemyConfig {
    fn default() -> Self {
        Self {
            rule_splits: 100,
            optimization_rounds_per_split: 5,
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
            action_change_multiplier: 4,
            default_action: Action {
                window_multiplier: 1.,
                window_increment: 1,
                intersend_ms: 0.01,
            },
            evaluation_config: EvaluationConfig::default(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct RemyDna {
    tree: RuleTree,
}

impl RemyDna {
    #[must_use]
    pub fn action<'a, O, const COUNT: bool>(
        &'a self,
        point: &Point,
        rule_override: &'a O,
    ) -> &Action
    where
        O: RuleOverride,
    {
        self.tree
            .action::<O, COUNT>(point, rule_override)
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
        for _ in 0..=self.config.rule_splits {}
        progress_handler.update_progress(1., Some(&result));
        result
    }
}
