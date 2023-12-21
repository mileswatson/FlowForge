use std::convert::Into;

use anyhow::Result;
use protobuf::Message;
use serde::{Deserialize, Serialize};

use crate::{
    flow::UtilityFunction, network::config::NetworkConfig, rand::Rng, Dna, ProgressHandler, Trainer,
};

pub mod rule_tree;

use self::{autogen::remy_dna::WhiskerTree, rule_tree::RuleTree};

#[allow(clippy::all, clippy::pedantic, clippy::nursery)]
mod autogen {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

#[derive(Serialize, Deserialize, Default)]
pub struct RemyConfig {}

type Type = RuleTree;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct RemyDna {
    tree: Type,
}

impl Dna for RemyDna {
    const NAME: &'static str = "remy";
    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(WhiskerTree::from(self.tree.clone()).write_to_bytes()?)
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
