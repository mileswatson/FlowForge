use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use ordered_float::NotNan;
use protobuf::Message;
use serde::{Deserialize, Serialize};

use crate::{
    evaluator::{EvaluationConfig, PopulateComponents},
    flow::{Flow, FlowProperties, NoActiveFlows, UtilityFunction},
    logging::NothingLogger,
    network::{
        config::NetworkConfig, protocols::remy::LossySender, toggler::Toggle, NetworkSlots, Packet,
    },
    rand::Rng,
    simulation::{DynComponent, MaybeHasVariant},
    time::Float,
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
    rule_tree::{BaseRuleTree, CountingRuleTree, LeafHandle, RuleTree},
};

#[allow(clippy::all, clippy::pedantic, clippy::nursery)]
mod autogen {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RemyConfig {
    pub rule_splits: u32,
    pub optimization_rounds_per_split: u32,
    pub min_action: Action,
    pub max_action: Action,
    pub initial_action_change: Action,
    pub max_action_change: Action,
    pub action_change_multiplier: i32,
    pub default_action: Action,
    pub evaluation_config: EvaluationConfig,
}

impl Default for RemyConfig {
    fn default() -> Self {
        Self {
            rule_splits: 100,
            optimization_rounds_per_split: 5,
            min_action: Action {
                window_multiplier: 0.,
                window_increment: 0,
                intersend_delay: 0.000_25.into(),
            },
            max_action: Action {
                window_multiplier: 1.,
                window_increment: 256,
                intersend_delay: 0.003.into(),
            },
            initial_action_change: Action {
                window_multiplier: 0.01,
                window_increment: 1,
                intersend_delay: 0.000_05.into(),
            },
            max_action_change: Action {
                window_multiplier: 0.5,
                window_increment: 32,
                intersend_delay: 0.001.into(),
            },
            action_change_multiplier: 4,
            default_action: Action {
                window_multiplier: 1.,
                window_increment: 1,
                intersend_delay: 0.003.into(),
            },
            evaluation_config: EvaluationConfig::default(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct RemyDna<const TESTING: bool = false> {
    tree: BaseRuleTree<TESTING>,
}

impl RemyDna {
    #[must_use]
    pub fn default(dna: &RemyConfig) -> Self {
        RemyDna {
            tree: BaseRuleTree::default(dna),
        }
    }
}

impl<const TESTING: bool> Dna for RemyDna<TESTING> {
    const NAME: &'static str = "remy";
    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(self.tree.to_whisker_tree().write_to_bytes()?)
    }

    fn deserialize(buf: &[u8]) -> Result<RemyDna<TESTING>> {
        Ok(RemyDna {
            tree: BaseRuleTree::<TESTING>::from_whisker_tree(&WhiskerTree::parse_from_bytes(buf)?),
        })
    }
}

impl RuleTree for RemyDna {
    fn action(&self, point: &Point) -> Option<&Action> {
        self.tree.action(point)
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

impl<T> PopulateComponents for T
where
    T: RuleTree + Sync,
{
    fn populate_components<'a>(
        &'a self,
        network_slots: NetworkSlots<'a, '_>,
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
    F: for<'a> Fn(&'a mut BaseRuleTree, &mut Rng) -> (Float, FlowProperties, CountingRuleTree<'a>),
{
    f
}

impl Trainer for RemyTrainer {
    type Config = RemyConfig;
    type Dna = RemyDna;

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
            let (s, props) = self
                .config
                .evaluation_config
                .evaluate(network_config, &counting_tree, utility_function, rng)
                .expect("Simulation to have active flows");
            (s, props, counting_tree)
        });
        let evaluate_action = |leaf: &LeafHandle, action: Action, rng: &mut Rng| {
            self.config
                .evaluation_config
                .evaluate(
                    network_config,
                    &leaf.augmented_tree(action),
                    utility_function,
                    rng,
                )
                .expect("Simulation to have active flows")
        };
        let mut dna = RemyDna::default(&self.config);
        let (mut score, mut props, mut counts) = evaluate_and_count(&mut dna.tree, rng);
        for i in 0..=self.config.rule_splits {
            if i == 0 {
                println!("Starting optimization.");
            } else {
                let (fraction_used, leaf) = counts.most_used_rule();
                println!(
                    "Split rule {:?} with usage {:.2}%",
                    leaf.domain(),
                    fraction_used * 100.
                );
                leaf.split();
                (score, props, counts) = evaluate_and_count(&mut dna.tree, rng);
            }
            for optimization_round in 0..self.config.optimization_rounds_per_split {
                println!(
                    "  Starting optimization round {}/{}",
                    optimization_round + 1,
                    self.config.optimization_rounds_per_split
                );
                while let Some((fraction_used, mut leaf)) = counts.most_used_unoptimized_rule() {
                    if fraction_used == 0. {
                        println!("    Skipped remaining rules with 0% usage");
                        break;
                    }
                    println!(
                        "    Optimizing {:?} with usage {:.2}%",
                        leaf.domain(),
                        fraction_used * 100.
                    );
                    println!("      Currently {:?}", leaf.action());
                    while let Some((s, _, new_action)) = leaf
                        .action()
                        .possible_improvements(&self.config)
                        .map(|action| {
                            let (s, props) = evaluate_action(&leaf, action.clone(), rng);
                            (s, props, action)
                        })
                        .filter(|(s, _, _)| s > &score)
                        .max_by_key(|(s, _, _)| NotNan::new(*s).unwrap())
                    {
                        println!("      Changed to {new_action:?}");
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
                    (score, _, counts) = evaluate_and_count(&mut dna.tree, rng);
                }
                dna.tree.mark_all_unoptimized();
                (score, props, counts) = evaluate_and_count(&mut dna.tree, rng);
            }
            println!("Achieved score {score:.2} with properties {props:?}");
        }
        progress_handler.update_progress(1., Some(&dna));
        dna
    }

    fn evaluate(
        &self,
        d: &RemyDna,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        rng: &mut Rng,
    ) -> Result<(Float, FlowProperties), NoActiveFlows> {
        self.config
            .evaluation_config
            .evaluate(network_config, d, utility_function, rng)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::{
        evaluator::EvaluationConfig, flow::AlphaFairness, network::config::NetworkConfig,
        rand::Rng, trainers::remy::RemyDna, Trainer,
    };

    use super::{RemyConfig, RemyTrainer};

    #[test]
    #[ignore = "Long runtime."]
    fn determinism() {
        let rng = Rng::from_seed(123_456);
        let remy_config = RemyConfig {
            rule_splits: 10,
            optimization_rounds_per_split: 1,
            evaluation_config: EvaluationConfig {
                network_samples: 10,
                ..EvaluationConfig::default()
            },
            ..RemyConfig::default()
        };
        let evaluate = || {
            let mut rng = rng.clone();
            let trainer = RemyTrainer::new(&remy_config);
            let dna = trainer.train(
                &NetworkConfig::default(),
                &AlphaFairness::PROPORTIONAL_THROUGHPUT_DELAY_FAIRNESS,
                &mut |_, _: Option<&RemyDna>| {},
                &mut rng,
            );
            trainer.evaluate(
                &dna,
                &NetworkConfig::default(),
                &AlphaFairness::PROPORTIONAL_THROUGHPUT_DELAY_FAIRNESS,
                &mut rng,
            )
        };

        assert_eq!(evaluate(), evaluate());
    }
}
