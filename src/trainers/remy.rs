use std::{iter::successors, ops::Mul};

use indicatif::{ParallelProgressIterator, ProgressBar};
use itertools::Itertools;
use ordered_float::NotNan;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    ccas::remy::{
        action::Action,
        dna::RemyDna,
        rule_tree::{CountingRuleTree, LeafHandle},
        RemyCcaTemplate,
    },
    eval::EvaluationConfig,
    flow::UtilityFunction,
    quantities::{milliseconds, seconds, Float, TimeSpan},
    util::{rand::Rng, WithLifetime},
    NetworkConfig, ProgressHandler, Trainer,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct RemyTrainer {
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

impl Default for RemyTrainer {
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

fn changes<T, U>(
    initial_change: T,
    max_change: T,
    multiplier: i32,
) -> impl Iterator<Item = T> + Clone
where
    T: PartialOrd + Copy + 'static,
    U: From<i32> + Mul<T, Output = T>,
{
    successors(Some(initial_change), move |x| {
        Some(U::from(multiplier) * *x)
    })
    .take_while(move |x| x <= &max_change)
    .flat_map(|x| [x, U::from(-1) * x])
}

impl RemyTrainer {
    #[must_use]
    pub fn possible_improvements(&self, action: Action) -> Vec<Action> {
        let RemyTrainer {
            min_action,
            max_action,
            initial_action_change,
            max_action_change,
            action_change_multiplier,
            ..
        } = self;
        changes::<Float, Float>(
            initial_action_change.window_multiplier,
            max_action_change.window_multiplier,
            *action_change_multiplier,
        )
        .cartesian_product(changes::<i32, i32>(
            initial_action_change.window_increment,
            max_action_change.window_increment,
            *action_change_multiplier,
        ))
        .cartesian_product(changes::<TimeSpan, Float>(
            initial_action_change.intersend_delay,
            max_action_change.intersend_delay,
            *action_change_multiplier,
        ))
        .map(
            move |((window_multiplier, window_increment), intersend_ms)| {
                &action
                    + &Action {
                        window_multiplier,
                        window_increment,
                        intersend_delay: intersend_ms,
                    }
            },
        )
        .filter(move |x| {
            min_action.window_multiplier <= x.window_multiplier
                && x.window_multiplier <= max_action.window_multiplier
                && min_action.window_increment <= x.window_increment
                && x.window_increment <= max_action.window_increment
                && min_action.intersend_delay <= x.intersend_delay
                && x.intersend_delay <= max_action.intersend_delay
        })
        .collect_vec()
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
    type Dna = RemyDna;
    type CcaTemplate<'a> = RemyCcaTemplate<&'a RemyDna>;

    #[allow(clippy::too_many_lines)]
    fn train<G: WithLifetime, H: ProgressHandler<RemyDna>>(
        &self,
        starting_point: Option<RemyDna>,
        network_config: &impl NetworkConfig<G>,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> RemyDna {
        let new_eval_rng = rng.identical_child_factory();
        let eval_and_count = coerce(|dna: &mut RemyDna| {
            let counting_tree = CountingRuleTree::new(&mut dna.tree);
            self.count_rule_usage_config
                .evaluate::<_, G, _>(
                    RemyCcaTemplate::default().with_not_sync(&counting_tree),
                    network_config,
                    utility_function,
                    &mut new_eval_rng(),
                )
                .expect("Simulation to have active flows");
            counting_tree
        });
        let test_new_action = |leaf: &LeafHandle, new_action: Action, mut rng: Rng| {
            self.change_eval_config
                .evaluate::<_, G, _>(
                    RemyCcaTemplate::default().with_not_sync(&leaf.augmented_tree(new_action)),
                    network_config,
                    utility_function,
                    &mut rng,
                )
                .expect("Simulation to have active flows")
        };
        let mut dna =
            starting_point.unwrap_or_else(|| RemyDna::default(self.default_action.clone()));
        for i in 0..=self.rule_splits {
            let frac = f64::from(i) / f64::from(self.rule_splits + 1);
            progress_handler.update_progress(frac, &dna);
            if i == 0 {
                println!("Starting optimization");
            } else {
                let mut counts = eval_and_count(&mut dna);
                if self.drill_down && counts.num_used_rules() <= 1 {
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
            for optimization_round in 0..self.optimization_rounds_per_split {
                println!(
                    "  Starting optimization round {}/{}",
                    optimization_round + 1,
                    self.optimization_rounds_per_split
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
                        let possible_improvements =
                            self.possible_improvements(leaf.action().clone());
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
        eval::EvaluationConfig,
        flow::AlphaFairness,
        networks::DefaultNetworkConfig,
        quantities::seconds,
        trainers::{remy::RemyDna, DefaultEffect},
        util::rand::Rng,
        Trainer,
    };

    use super::RemyTrainer;

    #[test]
    #[ignore = "long runtime"]
    fn determinism() {
        let mut rng = Rng::from_seed(123_456);
        let trainer = RemyTrainer {
            rule_splits: 1,
            optimization_rounds_per_split: 1,
            action_change_multiplier: 16,
            count_rule_usage_config: EvaluationConfig {
                network_samples: 100,
                run_sim_for: seconds(10.),
            },
            ..RemyTrainer::default()
        };
        let result = trainer.train::<DefaultEffect<'static>, _>(
            None,
            &DefaultNetworkConfig::default(),
            &AlphaFairness::PROPORTIONAL_THROUGHPUT_DELAY_FAIRNESS,
            &mut |_, _: &RemyDna| {},
            &mut rng,
        );
        insta::assert_yaml_snapshot!(result);
    }
}
