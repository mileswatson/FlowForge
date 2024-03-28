use std::{cell::RefCell, f64::NAN, marker::PhantomData};

use anyhow::Result;
use indicatif::{ParallelProgressIterator, ProgressBar};
use itertools::Itertools;
use ordered_float::NotNan;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    core::{logging::NothingLogger, meters::FlowMeter, rand::Rng},
    evaluator::EvaluationConfig,
    flow::{FlowProperties, NoActiveFlows, UtilityFunction},
    network::{
        config::NetworkConfig,
        senders::{
            remy::LossyRemySender,
            window::{LossyInternalControllerEffect, LossyInternalSenderEffect, LossySenderEffect},
        },
        toggler::Toggle,
        AddFlows, EffectTypeGenerator, Packet,
    },
    protocols::remy::{
        action::Action,
        dna::RemyDna,
        rule_tree::{AugmentedRuleTree, CountingRuleTree, LeafHandle, RuleTree},
    },
    quantities::{milliseconds, seconds, Float},
    simulation::{Address, HasSubEffect, SimulatorBuilder},
    trainers::DefaultEffect,
    ProgressHandler, Trainer,
};

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
                network_samples: 50,
                run_sim_for: seconds(60.),
            },
            evaluation_config: EvaluationConfig::default(),
        }
    }
}

pub struct RemyTrainer {
    config: RemyConfig,
}

pub struct RemyFlowAdder<T>(PhantomData<T>);

impl<G, T> AddFlows<G> for RemyFlowAdder<T>
where
    T: RuleTree,
    G: EffectTypeGenerator,
    for<'sim> G::Type<'sim>: HasSubEffect<LossySenderEffect<'sim, G::Type<'sim>>>
        + HasSubEffect<LossyInternalSenderEffect<'sim, G::Type<'sim>>>
        + HasSubEffect<LossyInternalControllerEffect>,
{
    type Dna = T;

    fn add_flows<'sim, 'a, F>(
        dna: &'a T,
        flows: impl IntoIterator<Item = F>,
        simulator_builder: &mut SimulatorBuilder<'sim, 'a, G::Type<'sim>>,
        sender_link_id: Address<'sim, Packet<'sim, G::Type<'sim>>, G::Type<'sim>>,
        _rng: &mut Rng,
    ) -> Vec<Address<'sim, Toggle, G::Type<'sim>>>
    where
        F: FlowMeter + 'a,
        G::Type<'sim>: 'sim,
        'sim: 'a,
    {
        let senders = flows
            .into_iter()
            .map(|flow| {
                let slot = LossyRemySender::reserve_slot::<_, NothingLogger>(simulator_builder);
                let address = slot.address();
                let packet_address = address.clone().cast();
                slot.set(
                    packet_address.clone(),
                    sender_link_id.clone(),
                    packet_address,
                    dna,
                    true,
                    flow,
                    NothingLogger,
                );
                address.cast()
            })
            .collect_vec();
        senders
    }
}

/// Hack until <https://github.com/rust-lang/rust/issues/97362> is stabilised
const fn coerce<F>(f: F) -> F
where
    F: for<'a> Fn(&'a mut RemyDna) -> CountingRuleTree<'a>,
{
    f
}

impl Trainer for RemyTrainer {
    type Config = RemyConfig;
    type Dna = RemyDna;
    type DefaultEffectGenerator = DefaultEffect<'static>;
    type DefaultFlowAdder = RemyFlowAdder<RemyDna>;

    fn new(config: &RemyConfig) -> RemyTrainer {
        RemyTrainer {
            config: config.clone(),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn train<H: ProgressHandler<RemyDna>>(
        &self,
        starting_point: Option<RemyDna>,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> RemyDna {
        let prog_handler = RefCell::new(progress_handler);
        let new_eval_rng = rng.identical_child_factory();
        let eval = |dna: &RemyDna| {
            let (score, props) = self
                .config
                .evaluation_config
                .evaluate::<RemyFlowAdder<RemyDna>, DefaultEffect>(
                    network_config,
                    dna,
                    utility_function,
                    &mut new_eval_rng(),
                )
                .expect("Simulation to have active flows");
            prog_handler.borrow_mut().update_progress(
                Some(dna),
                score,
                props.average_throughput,
                props.average_rtt.unwrap_or(seconds(NAN)),
            );
        };
        let eval_and_count = coerce(|dna: &mut RemyDna| {
            let counting_tree = CountingRuleTree::new(&mut dna.tree);
            let (score, props) = self
                .config
                .evaluation_config
                .evaluate::<RemyFlowAdder<CountingRuleTree>, DefaultEffect>(
                    network_config,
                    &counting_tree,
                    utility_function,
                    &mut new_eval_rng(),
                )
                .expect("Simulation to have active flows");
            println!("    Achieved eval score {score:.2} with {props}");
            counting_tree
        });
        let test_new_action = |leaf: &LeafHandle, new_action: Action, mut rng: Rng| {
            self.config
                .training_config
                .evaluate::<RemyFlowAdder<AugmentedRuleTree>, DefaultEffect>(
                    network_config,
                    &leaf.augmented_tree(new_action),
                    utility_function,
                    &mut rng,
                )
                .expect("Simulation to have active flows")
        };
        let mut dna =
            starting_point.unwrap_or_else(|| RemyDna::default(self.config.default_action.clone()));
        for i in 0..=self.config.rule_splits {
            eval(&dna);
            if i == 0 {
                println!("Starting optimization");
            } else {
                let (fraction_used, leaf) = eval_and_count(&mut dna).most_used_rule();
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
                    eval_and_count(&mut dna).most_used_unoptimized_rule()
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
                    let new_training_rng = rng.identical_child_factory();
                    let (mut best_score, _) = {
                        let original_action = leaf.action().clone();
                        test_new_action(&leaf, original_action, new_training_rng())
                    };
                    println!(
                        "      Currently {} with training score {best_score:.2}",
                        leaf.action()
                    );
                    while let Some((s, new_action)) = {
                        let possible_improvements = leaf
                            .action()
                            .possible_improvements(&self.config)
                            .collect_vec();
                        let progress = ProgressBar::new(possible_improvements.len() as u64);
                        possible_improvements
                            .into_par_iter()
                            .map(|action| {
                                let (s, _) =
                                    test_new_action(&leaf, action.clone(), new_training_rng());
                                (s, action)
                            })
                            .progress_with(progress)
                            .filter(|(s, _)| s > &best_score)
                            .max_by_key(|(s, _)| NotNan::new(*s).unwrap())
                    } {
                        println!(
                            "      Changed to {new_action} with training score {best_score:.2}"
                        );
                        best_score = s;
                        *leaf.action() = new_action;
                    }
                    leaf.mark_optimized();
                }
                dna.tree.mark_all_unoptimized();
            }
        }
        eval_and_count(&mut dna);
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
            .evaluate::<Self::DefaultFlowAdder, DefaultEffect>(
                network_config,
                d,
                utility_function,
                rng,
            )
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::{
        core::rand::Rng, evaluator::EvaluationConfig, flow::AlphaFairness,
        network::config::NetworkConfig, quantities::seconds, trainers::remy::RemyDna, Trainer,
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
                &mut |_: Option<&RemyDna>, _, _, _| {},
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
