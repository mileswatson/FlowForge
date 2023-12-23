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

#[derive(Serialize, Deserialize, Default)]
pub struct RemyConfig {
    epochs: usize,
    run_sim_for: Float,
    networks_per_iter: usize,
}

#[derive(Default, Debug, PartialEq)]
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

pub struct RemyTrainer {}

impl Trainer<RemyDna> for RemyTrainer {
    type Config = RemyConfig;

    fn new(config: &RemyConfig) -> RemyTrainer {
        RemyTrainer {}
    }

    fn train<H: ProgressHandler<RemyDna>>(
        &self,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> RemyDna {
        let result = RemyDna::default();
        progress_handler.update_progress(1., Some(&result));
        result
    }
}
