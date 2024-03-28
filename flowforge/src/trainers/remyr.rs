use std::{
    cell::RefCell,
    f32::{consts::PI, NAN},
    ops::Mul,
};

use dfdx::prelude::*;
use generativity::make_guard;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    core::{meters::CurrentFlowMeter, rand::Rng},
    evaluator::EvaluationConfig,
    protocols::{
        remy::{action::Action, point::Point, rule_tree::RuleTree},
        remyr::{
            dna::RemyrDna,
            net::{AsPolicyNetRef, CopyToDevice, HiddenLayers, PolicyNetwork, ACTION, STATE},
        },
    },
    quantities::{milliseconds, seconds, Time, TimeSpan},
    Trainer,
};

use super::{remy::RemyFlowAdder, DefaultEffect};

#[derive(Clone, Serialize, Deserialize)]
pub struct RemyrConfig {
    pub iters: u32,
    pub updates_per_iter: u32,
    pub min_point: Point,
    pub max_point: Point,
    pub min_action: Action,
    pub max_action: Action,
    pub hidden_layers: HiddenLayers,
    pub learning_rate: f64,
    pub learning_rate_annealing: bool,
    pub weight_decay: Option<f64>,
    pub clip: f32,
    pub gamma: f32,
    pub bandwidth_half_life: TimeSpan,
    pub training_config: EvaluationConfig,
    pub evaluation_config: EvaluationConfig,
}

impl Default for RemyrConfig {
    fn default() -> Self {
        Self {
            iters: 1000,
            updates_per_iter: 10,
            min_point: Point {
                ack_ewma: milliseconds(0.),
                send_ewma: milliseconds(0.),
                rtt_ratio: 1.,
            },
            max_point: Point {
                ack_ewma: seconds(1.),
                send_ewma: seconds(1.),
                rtt_ratio: 10.,
            },
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
            training_config: EvaluationConfig {
                network_samples: 30,
                run_sim_for: seconds(60.),
            },
            evaluation_config: EvaluationConfig::default(),
            hidden_layers: HiddenLayers(32, 16),
            learning_rate: 0.0003,
            learning_rate_annealing: true,
            weight_decay: Some(0.001),
            bandwidth_half_life: milliseconds(100.),
            clip: 0.2,
            gamma: 0.999,
        }
    }
}

impl RemyrConfig {
    fn initial_dna(&self, policy: PolicyNetwork<Cpu>) -> RemyrDna {
        RemyrDna {
            min_point: self.min_point.clone(),
            max_point: self.max_point.clone(),
            min_action: self.min_action.clone(),
            max_action: self.max_action.clone(),
            stddev_multiplier: NAN,
            policy,
        }
    }
}

#[derive(Debug)]
struct Record {
    observation: [f32; STATE],
    action: [f32; ACTION],
    action_log_prob: f32,
    reward_after_action: f32,
    reward_to_go_before_action: f32,
}

#[derive(Debug)]
struct Trajectory {
    records: Vec<Record>,
}

impl Trajectory {
    fn fill_reward_to_go(&mut self, gamma: f32) {
        let mut discounted_reward = 0.;
        for point in self.records.iter_mut().rev() {
            discounted_reward = point.reward_after_action + discounted_reward.mul(gamma);
            point.reward_to_go_before_action = discounted_reward;
        }
    }
}

struct RolloutResult<D: Device<f32>> {
    observations: Tensor<(usize, Const<STATE>), f32, D>,
    actions: Tensor<(usize, Const<ACTION>), f32, D>,
    action_log_probs: Tensor<(usize,), f32, D>,
    rewards_to_go_before_action: Tensor<(usize,), f32, D>,
}

impl<D: Device<f32>> RolloutResult<D> {
    pub fn new(trajectories: &[Trajectory], dev: &D) -> Self {
        let num_timesteps = trajectories.iter().map(|x| x.records.len()).sum();
        let observations = trajectories
            .iter()
            .flat_map(|x| x.records.iter())
            .flat_map(|x| x.observation)
            .collect();
        let actions = trajectories
            .iter()
            .flat_map(|x| x.records.iter())
            .flat_map(|x| x.action)
            .collect();
        let action_log_probs = trajectories
            .iter()
            .flat_map(|x| x.records.iter())
            .map(|x| x.action_log_prob)
            .collect();
        let rewards_to_go_before_action = trajectories
            .iter()
            .flat_map(|x| x.records.iter())
            .map(|x| x.reward_to_go_before_action)
            .collect();
        RolloutResult {
            observations: dev.tensor_from_vec(observations, (num_timesteps, Const::<STATE>)),
            actions: dev.tensor_from_vec(actions, (num_timesteps, Const::<ACTION>)),
            action_log_probs: dev.tensor_from_vec(action_log_probs, (num_timesteps,)),
            rewards_to_go_before_action: dev
                .tensor_from_vec(rewards_to_go_before_action, (num_timesteps,)),
        }
    }
}

fn calculate_action_log_probs<S: Dim, D: Device<f32>, T: Tape<f32, D>>(
    actions: Tensor<(S, Const<ACTION>), f32, D>,
    means: Tensor<(S, Const<ACTION>), f32, D, T>,
    stddevs: Tensor<(S, Const<ACTION>), f32, D, T>,
) -> Tensor<(S,), f32, D, T> {
    (((means - actions) / stddevs.with_empty_tape()).square() + stddevs.ln() * 2. + (2. * PI).ln())
        .sum::<(S,), Axis<1>>()
        * -0.5
}

pub struct RemyrRecorder<'a, P, U, F> {
    dna: &'a RemyrDna<P>,
    current_utility: U,
    record: F,
}

impl<'a, P, U, F> std::fmt::Debug for RemyrRecorder<'a, P, U, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RemyrRecordWrapper").finish()
    }
}

impl<'d, P, U, F> RuleTree for RemyrRecorder<'d, P, U, F>
where
    P: AsPolicyNetRef,
    F: Fn(Record),
    U: Fn(Time) -> f32,
{
    type Action<'b> = Action where Self: 'b;

    fn action(&self, point: &Point, time: Time) -> Option<Action> {
        Some(
            self.dna
                .probabilistic_action(point, |observation, action, action_log_prob| {
                    (self.record)(Record {
                        observation,
                        action,
                        action_log_prob,
                        reward_after_action: (self.current_utility)(time),
                        reward_to_go_before_action: NAN,
                    });
                }),
        )
    }
}

fn rollout(
    dna: &RemyrDna,
    network_config: &crate::network::config::NetworkConfig,
    utility_function: &dyn crate::flow::UtilityFunction,
    training_config: &EvaluationConfig,
    half_life: TimeSpan,
    gamma: f32,
    rng: &mut Rng,
) -> Vec<Trajectory> {
    let networks = (0..training_config.network_samples)
        .map(|_| (rng.sample(network_config), rng.create_child()))
        .collect_vec();

    networks
        .into_par_iter()
        .map(|(n, mut rng)| {
            let records = RefCell::new(Vec::new());
            make_guard!(guard);
            let flows = (0..n.num_senders)
                .map(|_| RefCell::new(CurrentFlowMeter::new_disabled(Time::SIM_START, half_life)))
                .collect_vec();
            let dna = RemyrRecorder {
                dna,
                current_utility: |time| {
                    let flow_stats = flows
                        .iter()
                        .filter_map(|x| x.borrow().current_properties(time).ok())
                        .collect_vec();
                    #[allow(clippy::cast_possible_truncation)]
                    return utility_function.total_utility(&flow_stats).unwrap().0 as f32;
                },
                record: |r| records.borrow_mut().push(r),
            };
            let sim = n.to_sim::<RemyFlowAdder<_>, _, DefaultEffect>(
                guard,
                &mut rng,
                &flows,
                &dna,
                |_| {},
            );
            sim.run_for(training_config.run_sim_for);
            let mut trajectory = Trajectory {
                records: records.into_inner(),
            };
            trajectory.fill_reward_to_go(gamma);
            trajectory
        })
        .collect()
}

pub struct RemyrTrainer {
    config: RemyrConfig,
}

impl Trainer for RemyrTrainer {
    type Config = RemyrConfig;
    type Dna = RemyrDna;
    type DefaultEffectGenerator = DefaultEffect<'static>;
    type DefaultFlowAdder = RemyFlowAdder<RemyrDna>;

    fn new(config: &Self::Config) -> Self {
        RemyrTrainer {
            config: config.clone(),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn train<H>(
        &self,
        starting_point: Option<Self::Dna>,
        network_config: &crate::network::config::NetworkConfig,
        utility_function: &dyn crate::flow::UtilityFunction,
        progress_handler: &mut H,
        rng: &mut crate::core::rand::Rng,
    ) -> Self::Dna
    where
        H: crate::ProgressHandler<Self::Dna>,
    {
        assert!(
            starting_point.is_none(),
            "Starting point not supported for genetic trainer!"
        );
        let dev = AutoDevice::default();
        let mut policy = dev.build_module(self.config.hidden_layers.policy_arch());
        let mut critic = dev.build_module::<f32>(self.config.hidden_layers.critic_arch());

        let mut critic_gradients = critic.alloc_grads();
        let mut critic_optimizer = Adam::new(
            &critic,
            AdamConfig {
                lr: self.config.learning_rate,
                weight_decay: self.config.weight_decay.map(WeightDecay::Decoupled),
                ..Default::default()
            },
        );
        let mut policy_gradients = policy.alloc_grads();
        let mut policy_optimizer = Adam::new(
            &policy,
            AdamConfig {
                lr: self.config.learning_rate,
                weight_decay: self.config.weight_decay.map(WeightDecay::Decoupled),
                ..Default::default()
            },
        );

        let sim_dev = Cpu::default();

        for i in 0..self.config.iters {
            let frac = f64::from(i) / f64::from(self.config.iters);
            progress_handler.update_progress(
                frac,
                Some(&self.config.initial_dna(policy.copy_to(&sim_dev))),
            );
            if self.config.learning_rate_annealing {
                policy_optimizer.cfg.lr = (1.0 - frac) * self.config.learning_rate;
                critic_optimizer.cfg.lr = (1.0 - frac) * self.config.learning_rate;
            }

            let dna = self.config.initial_dna(policy.copy_to(&sim_dev));
            let trajectories: Vec<Trajectory> = rollout(
                &dna,
                network_config,
                utility_function,
                &self.config.training_config,
                self.config.bandwidth_half_life,
                self.config.gamma,
                rng,
            );
            #[allow(clippy::cast_precision_loss)]
            let average_reward = trajectories
                .iter()
                .flat_map(|x| &x.records)
                .map(|x| x.reward_after_action)
                .sum::<f32>()
                / trajectories.len() as f32;
            let RolloutResult {
                observations,
                actions,
                action_log_probs,
                rewards_to_go_before_action,
            } = RolloutResult::new(&trajectories, &dev);
            let estimated_values_k = critic.forward(observations.clone()); // V

            let advantages_k = {
                let shape = (estimated_values_k.shape().0,);
                rewards_to_go_before_action.clone() - estimated_values_k.reshape_like(&shape)
            };
            let advantages_k = (advantages_k.clone() - advantages_k.clone().mean().array())
                / (advantages_k.stddev(0.).array() + 1e-10);

            let mut final_policy_loss = 0.;
            let mut final_critic_loss = 0.;
            for _ in 0..self.config.updates_per_iter {
                let (means, stddevs) = policy.forward(observations.trace(policy_gradients));

                // let stddevs = stddevs * STDDEV_MULTIPLIER;

                let action_log_probs_i =
                    calculate_action_log_probs(actions.clone(), means, stddevs);

                let ratios = (action_log_probs_i.reshape_like(action_log_probs.shape())
                    - action_log_probs.clone())
                .exp();

                let policy_loss = (-minimum(
                    ratios.with_empty_tape() * advantages_k.clone(),
                    clamp(ratios, 1. - self.config.clip, 1. + self.config.clip)
                        * advantages_k.clone(),
                ))
                .sum();

                final_policy_loss = policy_loss.array();

                policy_gradients = policy_loss.backward();

                policy_optimizer
                    .update(&mut policy, &policy_gradients)
                    .unwrap();
                policy.zero_grads(&mut policy_gradients);
            }
            for _ in 0..self.config.updates_per_iter {
                let estimated_values_i =
                    critic.forward(observations.clone().traced(critic_gradients));

                let critic_loss = mse_loss(
                    estimated_values_i.reshape_like(rewards_to_go_before_action.shape()),
                    rewards_to_go_before_action.clone(),
                );
                final_critic_loss = critic_loss.array();

                critic_gradients = critic_loss.backward();
                critic_optimizer
                    .update(&mut critic, &critic_gradients)
                    .unwrap();
                critic.zero_grads(&mut critic_gradients);
            }
            println!(
                "{frac:>5.2} {final_policy_loss:>5.2} {final_critic_loss:>5.2} {average_reward:>5.2}"
            );
        }
        let dna = self.config.initial_dna(policy.copy_to(&sim_dev));
        progress_handler.update_progress(1., Some(&dna));
        dna
    }

    fn evaluate(
        &self,
        d: &Self::Dna,
        network_config: &crate::network::config::NetworkConfig,
        utility_function: &dyn crate::flow::UtilityFunction,
        rng: &mut crate::core::rand::Rng,
    ) -> anyhow::Result<
        (crate::quantities::Float, crate::flow::FlowProperties),
        crate::flow::NoActiveFlows,
    > {
        todo!()
    }
}
