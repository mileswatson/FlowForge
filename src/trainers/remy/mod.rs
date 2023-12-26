use std::{convert::Into, rc::Rc};

use anyhow::Result;
use protobuf::Message;
use serde::{Deserialize, Serialize};

use crate::{
    evaluator::{EvaluationConfig, PopulateComponents},
    flow::{Flow, UtilityFunction},
    network::{
        config::NetworkConfig, protocols::window::lossy_window::Packet, toggler::Toggle,
        NetworkSlots,
    },
    rand::Rng,
    simulation::MaybeHasVariant,
    Dna, ProgressHandler, Trainer,
};

pub mod action;
pub mod cube;
pub mod point;
pub mod rule_tree;

use self::{
    action::Action,
    autogen::remy_dna::WhiskerTree,
    point::Point,
    rule_tree::{NoOverride, RuleOverride, RuleTree},
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
    max_action_change: Action,
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
            max_action_change: Action {
                window_multiplier: 0.5,
                window_increment: 32,
                intersend_ms: 1.,
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

struct RemyNetwork<'a, O, const COUNT: bool> {
    dna: &'a RemyDna,
    rule_override: O,
}

impl<'a, O, E, const COUNT: bool> PopulateComponents<E> for RemyNetwork<'a, O, COUNT>
where
    O: Sync,
{
    fn populate_components(
        &self,
        network_slots: NetworkSlots<E>,
        rng: &mut Rng,
    ) -> Vec<Rc<dyn Flow>> {
        todo!()
    }
}

pub struct RemyTrainer {
    config: RemyConfig,
}

#[derive(Debug)]
pub enum RemyMessage {
    Packet(Packet),
    Toggle(Toggle),
}

impl MaybeHasVariant<Toggle> for RemyMessage {
    fn try_into(self) -> Result<Toggle, Self> {
        match self {
            RemyMessage::Packet(_) => Err(self),
            RemyMessage::Toggle(t) => Ok(t),
        }
    }
}

impl From<Toggle> for RemyMessage {
    fn from(value: Toggle) -> Self {
        RemyMessage::Toggle(value)
    }
}

impl MaybeHasVariant<Packet> for RemyMessage {
    fn try_into(self) -> Result<Packet, Self> {
        match self {
            RemyMessage::Packet(p) => Ok(p),
            RemyMessage::Toggle(_) => Err(self),
        }
    }
}

impl From<Packet> for RemyMessage {
    fn from(value: Packet) -> Self {
        RemyMessage::Packet(value)
    }
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
        let mut dna = RemyDna::default(&self.config);
        for _ in 0..=self.config.rule_splits {
            self.config
                .evaluation_config
                .evaluate::<RemyMessage, Packet>(
                    network_config,
                    &RemyNetwork::<_, true> {
                        dna: &dna,
                        rule_override: NoOverride,
                    },
                    utility_function,
                    rng,
                );
            while let Some(leaf) = dna.tree.most_used_unoptimized_rule() {
                leaf.optimized = true;
            }
        }
        progress_handler.update_progress(1., Some(&dna));
        dna
    }
}
