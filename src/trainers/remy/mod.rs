use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use indicatif::{ParallelProgressIterator, ProgressBar};
use itertools::Itertools;
use ordered_float::NotNan;
use protobuf::Message;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    evaluator::{EvaluationConfig, PopulateComponents},
    flow::{Flow, FlowProperties, NoActiveFlows, UtilityFunction},
    logging::NothingLogger,
    network::{
        config::NetworkConfig, protocols::remy::LossySender, toggler::Toggle, NetworkSlots, Packet,
    },
    quantities::{milliseconds, seconds, Float},
    rand::Rng,
    simulation::{DynComponent, MaybeHasVariant},
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
    pub training_config: EvaluationConfig,
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
                intersend_delay: milliseconds(0.25),
            },
            max_action: Action {
                window_multiplier: 1.,
                window_increment: 256,
                intersend_delay: milliseconds(3.),
            },
            initial_action_change: Action {
                window_multiplier: 0.01,
                window_increment: 1,
                intersend_delay: milliseconds(0.05),
            },
            max_action_change: Action {
                window_multiplier: 0.5,
                window_increment: 32,
                intersend_delay: milliseconds(1.),
            },
            action_change_multiplier: 4,
            default_action: Action {
                window_multiplier: 1.,
                window_increment: 1,
                intersend_delay: milliseconds(3.),
            },
            training_config: EvaluationConfig {
                network_samples: 100,
                run_sim_for: seconds(120.),
            },
            evaluation_config: EvaluationConfig {
                network_samples: 10000,
                run_sim_for: seconds(120.),
            },
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
pub enum RemyMessage<'sim> {
    Packet(Packet<'sim>),
    Toggle(Toggle),
}

impl<T> PopulateComponents for T
where
    T: RuleTree + Sync,
{
    fn populate_components<'sim, 'a>(
        &'a self,
        network_slots: NetworkSlots<'sim, 'a, '_>,
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

impl<'sim> MaybeHasVariant<'sim, Toggle> for RemyMessage<'sim> {
    fn try_into(self) -> Result<Toggle, Self> {
        match self {
            RemyMessage::Packet(_) => Err(self),
            RemyMessage::Toggle(t) => Ok(t),
        }
    }
}

impl<'sim> From<Toggle> for RemyMessage<'sim> {
    fn from(value: Toggle) -> Self {
        RemyMessage::Toggle(value)
    }
}

impl<'sim> MaybeHasVariant<'sim, Packet<'sim>> for RemyMessage<'sim> {
    fn try_into(self) -> Result<Packet<'sim>, Self> {
        match self {
            RemyMessage::Packet(p) => Ok(p),
            RemyMessage::Toggle(_) => Err(self),
        }
    }
}

impl<'sim> From<Packet<'sim>> for RemyMessage<'sim> {
    fn from(value: Packet<'sim>) -> Self {
        RemyMessage::Packet(value)
    }
}

/// Hack until <https://github.com/rust-lang/rust/issues/97362> is stabilised
const fn coerce<F>(f: F) -> F
where
    F: for<'a> Fn(&'a mut BaseRuleTree, &mut Rng) -> CountingRuleTree<'a>,
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
        starting_point: Option<RemyDna>,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> RemyDna {
        let eval_and_count = coerce(|tree: &mut BaseRuleTree, rng: &mut Rng| {
            let counting_tree = CountingRuleTree::new(tree);
            let (score, props) = self
                .config
                .evaluation_config
                .evaluate(network_config, &counting_tree, utility_function, rng)
                .expect("Simulation to have active flows");
            println!("    Achieved eval score {score:.2} with {props}");
            counting_tree
        });
        let training_eval = |leaf: &LeafHandle, new_action: Action, mut rng: Rng| {
            self.config
                .training_config
                .evaluate(
                    network_config,
                    &leaf.augmented_tree(new_action),
                    utility_function,
                    &mut rng,
                )
                .expect("Simulation to have active flows")
        };
        let mut dna = starting_point.unwrap_or_else(|| RemyDna::default(&self.config));
        for i in 0..=self.config.rule_splits {
            if i == 0 {
                println!("Starting optimization");
            } else {
                let (fraction_used, leaf) = eval_and_count(&mut dna.tree, rng).most_used_rule();
                println!(
                    "Split rule {} with usage {:.2}%",
                    leaf.domain(),
                    fraction_used * 100.
                );
                leaf.split();
            }
            for optimization_round in 0..self.config.optimization_rounds_per_split {
                println!(
                    "  Starting optimization round {}/{}",
                    optimization_round + 1,
                    self.config.optimization_rounds_per_split
                );
                while let Some((fraction_used, mut leaf)) =
                    eval_and_count(&mut dna.tree, rng).most_used_unoptimized_rule()
                {
                    if fraction_used == 0. {
                        println!("    Skipped remaining rules with 0% usage");
                        break;
                    }
                    println!(
                        "    Optimizing {} with usage {:.2}%",
                        leaf.domain(),
                        fraction_used * 100.
                    );
                    println!("      Created new training set");
                    let new_identical_rng = rng.identical_child_factory();
                    let (mut score, _) = {
                        let original_action = leaf.action().clone();
                        training_eval(&leaf, original_action, new_identical_rng())
                    };
                    println!(
                        "      Currently {} with training score {score:.2}",
                        leaf.action()
                    );
                    while let Some((s, _, new_action)) = {
                        let possible_improvements = leaf
                            .action()
                            .possible_improvements(&self.config)
                            .collect_vec();
                        let progress = ProgressBar::new(possible_improvements.len() as u64);
                        possible_improvements
                            .into_par_iter()
                            .map(|action| {
                                let (s, props) =
                                    training_eval(&leaf, action.clone(), new_identical_rng());
                                (s, props, action)
                            })
                            .progress_with(progress)
                            .filter(|(s, _, _)| s > &score)
                            .max_by_key(|(s, _, _)| NotNan::new(*s).unwrap())
                    } {
                        println!("      Changed to {new_action} with training score {score:.2}");
                        score = s;
                        *leaf.action() = new_action;
                    }
                    leaf.mark_optimized();
                }
                dna.tree.mark_all_unoptimized();
                progress_handler.update_progress(
                    f64::from(
                        i * self.config.optimization_rounds_per_split + optimization_round + 1,
                    ) / f64::from(
                        self.config.optimization_rounds_per_split
                            * self.config.optimization_rounds_per_split,
                    ),
                    Some(&dna),
                );
            }
        }
        eval_and_count(&mut dna.tree, rng);
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
        println!("Number of rule splits: {}", d.tree.num_parents());
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
        quantities::seconds, rand::Rng, trainers::remy::RemyDna, Trainer,
    };

    use super::{RemyConfig, RemyTrainer};

    #[test]
    #[ignore = "long runtime"]
    fn determinism() {
        let mut rng = Rng::from_seed(123_456);
        let new_identical_rng = rng.identical_child_factory();
        let remy_config = RemyConfig {
            rule_splits: 1,
            optimization_rounds_per_split: 1,
            evaluation_config: EvaluationConfig {
                network_samples: 32,
                run_sim_for: seconds(120.),
            },
            ..RemyConfig::default()
        };
        let evaluate = || {
            let mut rng = new_identical_rng();
            let trainer = RemyTrainer::new(&remy_config);
            let dna = trainer.train(
                None,
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
