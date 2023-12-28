use std::rc::Rc;

use anyhow::Result;
use ordered_float::NotNan;
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
    rule_tree::{CountingRuleTree, LeafHandle, RuleTree},
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
    tree: CountingRuleTree,
}

impl RemyDna {
    #[must_use]
    pub fn default(dna: &RemyConfig) -> Self {
        RemyDna {
            tree: CountingRuleTree::default(dna),
        }
    }
}

impl Dna for RemyDna {
    const NAME: &'static str = "remy";
    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(self.tree.to_whisker_tree().write_to_bytes()?)
    }

    fn deserialize(buf: &[u8]) -> Result<RemyDna> {
        Ok(RemyDna {
            tree: CountingRuleTree::from_whisker_tree(&WhiskerTree::parse_from_bytes(buf)?),
        })
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

impl<T, E> PopulateComponents<E> for T
where
    T: RuleTree + Sync,
{
    fn populate_components(
        &self,
        network_slots: NetworkSlots<E>,
        rng: &mut Rng,
    ) -> Vec<Rc<dyn Flow>> {
        todo!()
    }
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
        let evaluate_and_count_rule_uses = |dna: &mut RemyDna, rng: &mut Rng| {
            dna.tree.reset_counts();
            self.config
                .evaluation_config
                .evaluate::<RemyMessage, Packet>(network_config, &dna.tree, utility_function, rng)
        };
        let evaluate_action = |leaf: &LeafHandle, action: Action, rng: &mut Rng| {
            self.config
                .evaluation_config
                .evaluate::<RemyMessage, Packet>(
                    network_config,
                    &leaf.augmented_tree(action),
                    utility_function,
                    rng,
                )
        };
        let mut dna = RemyDna::default(&self.config);
        let mut score = evaluate_and_count_rule_uses(&mut dna, rng);
        for i in 0..=self.config.rule_splits {
            if i > 0 {
                dna.tree.most_used_rule().split();

                score = evaluate_and_count_rule_uses(&mut dna, rng);
            }
            while let Some(mut leaf) = dna.tree.most_used_unoptimized_rule() {
                while let Some(new_action) = leaf
                    .action()
                    .possible_improvements(&self.config)
                    .into_iter()
                    .map(|action| (evaluate_action(&leaf, action.clone(), rng), action))
                    .filter(|(s, _)| s > &score)
                    .max_by_key(|(s, _)| NotNan::new(*s).unwrap())
                    .map(|(_, action)| action)
                {
                    *leaf.action() = new_action;
                }
                leaf.mark_optimized();
                score = evaluate_and_count_rule_uses(&mut dna, rng);
            }
            dna.tree.mark_all_unoptimized();
        }
        progress_handler.update_progress(1., Some(&dna));
        dna
    }
}
