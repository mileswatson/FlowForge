use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use ordered_float::NotNan;
use protobuf::Message;
use serde::{Deserialize, Serialize};

use crate::{
    evaluator::{EvaluationConfig, PopulateComponents},
    flow::{Flow, UtilityFunction},
    logging::NothingLogger,
    network::{
        config::NetworkConfig,
        protocols::{remy::LossySender, window::lossy_window::Packet},
        toggler::Toggle,
        NetworkSlots,
    },
    rand::Rng,
    simulation::{DynComponent, HasVariant, MaybeHasVariant},
    Dna, ProgressHandler, Trainer,
};

pub mod action;
pub mod cube;
pub mod point;
pub mod rule_tree;

use self::{
    action::Action,
    autogen::remy_dna::WhiskerTree,
    rule_tree::{BaseRuleTree, CountingRuleTree, LeafHandle, RuleTree},
};

#[allow(clippy::all, clippy::pedantic, clippy::nursery)]
mod autogen {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RemyConfig {
    rule_splits: u32,
    optimization_rounds_per_split: u32,
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
    tree: BaseRuleTree,
}

impl RemyDna {
    #[must_use]
    pub fn default(dna: &RemyConfig) -> Self {
        RemyDna {
            tree: BaseRuleTree::default(dna),
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
            tree: BaseRuleTree::from_whisker_tree(&WhiskerTree::parse_from_bytes(buf)?),
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
    E: MaybeHasVariant<Toggle> + HasVariant<Packet>,
{
    fn populate_components<'a>(
        &'a self,
        network_slots: NetworkSlots<'a, '_, E>,
        _rng: &mut Rng,
    ) -> Vec<Rc<dyn Flow + 'a>> {
        network_slots
            .sender_slots
            .into_iter()
            .map(|slot| {
                let sender = Rc::new(RefCell::new(LossySender::new(
                    slot.id(),
                    network_slots.sender_link_id,
                    slot.id(),
                    self,
                    true,
                    NothingLogger,
                )));
                slot.set(DynComponent::Shared(sender.clone()));
                sender as Rc<dyn Flow>
            })
            .collect()
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

/// Hack until <https://github.com/rust-lang/rust/issues/97362> is stabilised
const fn coerce<F>(f: F) -> F
where
    F: for<'a> Fn(&'a mut BaseRuleTree, &mut Rng) -> (f64, CountingRuleTree<'a>),
{
    f
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
        let evaluate_and_count = coerce(|tree: &mut BaseRuleTree, rng: &mut Rng| {
            let counting_tree = CountingRuleTree::new(tree);
            let score = self
                .config
                .evaluation_config
                .evaluate::<RemyMessage, Packet>(
                    network_config,
                    &counting_tree,
                    utility_function,
                    rng,
                );
            (score, counting_tree)
        });
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
        let (mut score, mut counts) = evaluate_and_count(&mut dna.tree, rng);
        for i in 0..=self.config.rule_splits {
            if i > 0 {
                counts.most_used_rule().split();
                println!("Split rule!");
                (score, counts) = evaluate_and_count(&mut dna.tree, rng);
            }
            println!("Score: {score}");
            for optimization_round in 0..self.config.optimization_rounds_per_split {
                println!(
                    "Starting optimisation round {}/{}",
                    optimization_round + 1,
                    self.config.optimization_rounds_per_split
                );
                while let Some(mut leaf) = counts.most_used_unoptimized_rule() {
                    while let Some((s, new_action)) = leaf
                        .action()
                        .possible_improvements(&self.config)
                        .map(|action| {
                            let score = evaluate_action(&leaf, action.clone(), rng);
                            (score, action)
                        })
                        .filter(|(s, _)| s > &score)
                        .max_by_key(|(s, _)| NotNan::new(*s).unwrap())
                    {
                        println!("Improved score from {score} to {s} using {new_action:?}");
                        score = s;
                        *leaf.action() = new_action;
                    }
                    leaf.mark_optimized();
                    progress_handler.update_progress(
                        f64::from(
                            i * self.config.optimization_rounds_per_split + optimization_round,
                        ) / f64::from(
                            self.config.optimization_rounds_per_split
                                * self.config.optimization_rounds_per_split,
                        ),
                        Some(&dna),
                    );
                    (score, counts) = evaluate_and_count(&mut dna.tree, rng);
                    println!("Base: {score}");
                }
                dna.tree.mark_all_unoptimized();
                (score, counts) = evaluate_and_count(&mut dna.tree, rng);
            }
        }
        progress_handler.update_progress(1., Some(&dna));
        dna
    }
}
