use std::f32::consts::PI;

use dfdx::prelude::*;

use crate::{
    core::rand::Wrapper,
    quantities::{Time, TimeSpan},
};

use self::{
    dna::RemyrDna,
    net::{AsPolicyNetRef, PolicyNet, ACTION, STATE},
};

use super::remy::{action::Action, point::Point, rule_tree::RuleTree};

pub mod dna;
pub mod net;

fn point_to_tensor(dev: &Cpu, point: &Point) -> Tensor<(Const<STATE>,), f32, Cpu> {
    #[allow(clippy::cast_possible_truncation)]
    dev.tensor([
        point.ack_ewma.to_underlying() as f32,
        point.send_ewma.to_underlying() as f32,
        point.rtt_ratio.to_underlying() as f32,
    ])
}

fn action_to_tensor(dev: &Cpu, action: &Action) -> Tensor<(Const<ACTION>,), f32, Cpu> {
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_precision_loss)]
    dev.tensor([
        action.window_multiplier as f32,
        action.window_increment as f32,
        action.intersend_delay.to_underlying() as f32,
    ])
}

fn tensor_to_action(tensor: &Tensor<(Const<ACTION>,), f32, Cpu>) -> Action {
    let arr = tensor.array();
    #[allow(clippy::cast_possible_truncation)]
    Action {
        window_multiplier: f64::from(arr[0]),
        window_increment: arr[1].round() as i32,
        intersend_delay: TimeSpan::from_underlying(f64::from(arr[2])),
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

impl RemyrDna {
    pub fn raw_action<F>(&self, point: &Point, f: F) -> Action
    where
        F: FnOnce(Tensor1D<STATE>, (Tensor1D<ACTION>, Tensor1D<ACTION>)) -> Tensor1D<ACTION>,
    {
        let dev = self.policy.as_policy_net_ref().0 .0.weight.dev();
        let point = point_to_tensor(dev, point);
        let min_point = point_to_tensor(dev, &self.min_point);
        let max_point = point_to_tensor(dev, &self.max_point);
        let input = ((point - min_point.clone()) / (max_point - min_point)).clamp(0., 1.) * 2. - 1.;
        let (mean, stddev) = self.policy.as_policy_net_ref().forward(input.clone());
        let action = f(input, (mean, stddev));

        let max_action = action_to_tensor(dev, &self.max_action);
        let min_action = action_to_tensor(dev, &self.min_action);

        let action =
            min_action.clone() + (max_action - min_action) * (action.clamp(-1., 1.) + 1.) / 2.;

        tensor_to_action(&action)
    }

    fn deterministic_action(&self, point: &Point) -> Action {
        self.raw_action(point, |_, (mean, _stddev)| mean)
    }

    pub fn probabilistic_action<F>(&self, point: &Point, f: F) -> Action
    where
        F: FnOnce([f32; STATE], [f32; ACTION], f32),
    {
        self.raw_action(point, |observation, (mean, stddev)| {
            let action = mean.clone()
                + self.policy.as_policy_net_ref().device().sample_normal() * stddev.clone();
            let action_log_prob = calculate_action_log_probs::<Const<1>, _, _>(
                action.clone().reshape(),
                mean.reshape(),
                stddev.reshape(),
            );
            f(
                observation.array(),
                action.array(),
                action_log_prob.reshape::<()>().array(),
            );
            action
        })
    }
}

impl RuleTree for RemyrDna {
    type Action<'b> = Action where Self: 'b;

    fn action(&self, point: &Point, _time: Time) -> Option<Action> {
        Some(self.deterministic_action(point))
    }
}
