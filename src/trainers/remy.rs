use indicatif::{ParallelProgressIterator, ProgressBar};
use itertools::Itertools;
use ordered_float::NotNan;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    components::config::NetworkConfig,
    core::rand::Rng,
    evaluator::EvaluationConfig,
    flow::UtilityFunction,
    ccas::remy::{
        action::Action,
        dna::RemyDna,
        rule_tree::{CountingRuleTree, LeafHandle},
        RemyCcaTemplate, RuleTreeCcaTemplate,
    },
    quantities::{milliseconds, seconds},
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
    pub drill_down: bool,
}

impl Default for RemyConfig {
    fn default() -> Self {
        Self {
            rule_splits: 100,
            optimization_rounds_per_split: 2,
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
            drill_down: true,
        }
    }
}

pub struct RemyTrainer {
    config: RemyConfig,
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
    type CcaTemplate<'a> = RemyCcaTemplate<'a>;
    type DefaultEffectGenerator = DefaultEffect<'static>;

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
                .evaluate::<_, DefaultEffect>(
                    RuleTreeCcaTemplate::default().with_not_sync(&counting_tree),
                    network_config,
                    utility_function,
                    &mut new_eval_rng(),
                )
                .expect("Simulation to have active flows");
            counting_tree
        });
        let test_new_action = |leaf: &LeafHandle, new_action: Action, mut rng: Rng| {
            self.config
                .change_eval_config
                .evaluate::<_, DefaultEffect>(
                    RuleTreeCcaTemplate::default().with_not_sync(&leaf.augmented_tree(new_action)),
                    network_config,
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
                let mut counts = eval_and_count(&mut dna);
                if self.config.drill_down && counts.num_used_rules() <= 1 {
                    loop {
                        let (fraction_used, leaf) = counts.most_used_rule();
                        println!(
                            "Split rule {} with usage {:.2}%",
                            leaf.domain(),
                            fraction_used * 100.
                        );
                        leaf.split();
                        counts = eval_and_count(&mut dna);
                        if counts.num_used_rules() > 1 {
                            break;
                        }
                    }
                } else {
                    let (fraction_used, leaf) = counts.most_used_rule();
                    println!(
                        "Split rule {} with usage {:.2}%",
                        leaf.domain(),
                        fraction_used * 100.
                    );
                    leaf.split();
                }
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
    use crate::{
        components::config::NetworkConfig, core::rand::Rng, evaluator::EvaluationConfig,
        flow::AlphaFairness, quantities::seconds, trainers::remy::RemyDna, Trainer,
    };

    use super::{RemyConfig, RemyTrainer};

    #[test]
    #[ignore = "long runtime"]
    fn determinism() {
        let mut rng = Rng::from_seed(123_456);
        let remy_config = RemyConfig {
            rule_splits: 1,
            optimization_rounds_per_split: 1,
            action_change_multiplier: 16,
            count_rule_usage_config: EvaluationConfig {
                network_samples: 100,
                run_sim_for: seconds(10.),
            },
            ..RemyConfig::default()
        };
        let trainer = RemyTrainer::new(&remy_config);
        let result = trainer.train(
            None,
            &NetworkConfig::default(),
            &AlphaFairness::PROPORTIONAL_THROUGHPUT_DELAY_FAIRNESS,
            &mut |_, _: &RemyDna| {},
            &mut rng,
        );
        insta::assert_yaml_snapshot!(result);
    }
}
