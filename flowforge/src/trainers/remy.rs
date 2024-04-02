use indicatif::{ParallelProgressIterator, ProgressBar};
use itertools::Itertools;
use ordered_float::NotNan;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        logging::NothingLogger,
        meters::FlowMeter,
        rand::{DiscreteDistribution, Rng},
    },
    evaluator::EvaluationConfig,
    flow::UtilityFunction,
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
        rule_tree::{AugmentedRuleTree, CountingRuleTree, DynRuleTree, LeafHandle},
    },
    quantities::{milliseconds, seconds},
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
    pub change_eval_config: EvaluationConfig,
    pub count_rule_usage_config: EvaluationConfig,
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
            change_eval_config: EvaluationConfig {
                network_samples: 50,
                run_sim_for: seconds(60.),
            },
            count_rule_usage_config: EvaluationConfig::default(),
        }
    }
}

pub struct RemyTrainer {
    config: RemyConfig,
}

#[derive(Default)]
pub struct RemyFlowAdder {
    repeat_actions: Option<DiscreteDistribution<u32>>,
}

impl RemyFlowAdder {
    #[must_use]
    pub const fn new(repeat_actions: Option<DiscreteDistribution<u32>>) -> RemyFlowAdder {
        RemyFlowAdder { repeat_actions }
    }
}

impl<G, T> AddFlows<T, G> for RemyFlowAdder
where
    T: DynRuleTree,
    G: EffectTypeGenerator,
    for<'sim> G::Type<'sim>: HasSubEffect<LossySenderEffect<'sim, G::Type<'sim>>>
        + HasSubEffect<LossyInternalSenderEffect<'sim, G::Type<'sim>>>
        + HasSubEffect<LossyInternalControllerEffect>,
{
    fn add_flows<'sim, 'a, F>(
        &self,
        dna: T,
        flows: impl IntoIterator<Item = F>,
        simulator_builder: &mut SimulatorBuilder<'sim, 'a, G::Type<'sim>>,
        sender_link_id: Address<'sim, Packet<'sim, G::Type<'sim>>, G::Type<'sim>>,
        rng: &mut Rng,
    ) -> Vec<Address<'sim, Toggle, G::Type<'sim>>>
    where
        T: Clone + 'a,
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
                    dna.clone(),
                    true,
                    flow,
                    self.repeat_actions.clone(),
                    rng.create_child(),
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
    type DefaultFlowAdder<'a> = RemyFlowAdder;

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
        let new_eval_rng = rng.identical_child_factory();
        let eval_and_count = coerce(|dna: &mut RemyDna| {
            let counting_tree = CountingRuleTree::new(&mut dna.tree);
            self.config
                .count_rule_usage_config
                .evaluate::<&CountingRuleTree, DefaultEffect>(
                    &RemyFlowAdder::default(),
                    network_config,
                    &counting_tree,
                    utility_function,
                    &mut new_eval_rng(),
                )
                .expect("Simulation to have active flows");
            counting_tree
        });
        let test_new_action = |leaf: &LeafHandle, new_action: Action, mut rng: Rng| {
            self.config
                .change_eval_config
                .evaluate::<&AugmentedRuleTree, DefaultEffect>(
                    &RemyFlowAdder::default(),
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
            let frac = f64::from(i) / f64::from(self.config.rule_splits + 1);
            progress_handler.update_progress(frac, &dna);
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
            count_rule_usage_config: EvaluationConfig {
                network_samples: 32,
                run_sim_for: seconds(120.),
            },
            ..RemyConfig::default()
        };
        let evaluate = || {
            let mut rng = new_identical_rng();
            let trainer = RemyTrainer::new(&remy_config);
            trainer.train(
                None,
                &NetworkConfig::default(),
                &AlphaFairness::PROPORTIONAL_THROUGHPUT_DELAY_FAIRNESS,
                &mut |_, _: &RemyDna| {},
                &mut rng,
            )
        };

        assert_eq!(evaluate(), evaluate());
    }
}
