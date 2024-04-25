use dfdx::prelude::*;

use crate::{
    quantities::{Time, TimeSpan},
    util::rand::Wrapper,
};

use self::{
    dna::RemyrDna,
    net::{AsPolicyNetRef, ACTION, OBSERVATION},
};

use super::remy::{action::Action, point::Point, RemyPolicy};

pub mod dna;
pub mod net;

fn point_to_tensor(dev: &Cpu, point: &Point) -> Tensor<(Const<OBSERVATION>,), f32, Cpu> {
    dev.tensor([
        point.ack_ewma.to_underlying() as f32,
        point.send_ewma.to_underlying() as f32,
        point.rtt_ratio.to_underlying() as f32,
    ])
}

fn action_to_tensor(dev: &Cpu, action: &Action) -> Tensor<(Const<ACTION>,), f32, Cpu> {
    #[allow(clippy::cast_precision_loss)]
    dev.tensor([
        action.window_multiplier as f32,
        action.window_increment as f32,
        action.intersend_delay.to_underlying() as f32,
    ])
}

fn tensor_to_action(tensor: &Tensor<(Const<ACTION>,), f32, Cpu>) -> Action {
    let arr = tensor.array();
    Action {
        window_multiplier: f64::from(arr[0]),
        window_increment: arr[1].round() as i32,
        intersend_delay: TimeSpan::from_underlying(f64::from(arr[2])),
    }
}

impl RemyrDna {
    pub fn raw_action<F>(&self, point: &Point, f: F) -> Action
    where
        F: FnOnce(Tensor1D<OBSERVATION>, Tensor1D<ACTION>) -> Tensor1D<ACTION>,
    {
        let dev = self.policy.as_policy_net_ref().0 .0.weight.dev();
        let point = point_to_tensor(dev, point);
        let min_point = point_to_tensor(dev, &self.min_point);
        let max_point = point_to_tensor(dev, &self.max_point);
        let input = ((point - min_point.clone()) / (max_point - min_point)).clamp(0., 1.) * 2. - 1.;
        let mean = self.policy.as_policy_net_ref().forward(input.clone());
        let action = f(input, mean);

        let max_action = action_to_tensor(dev, &self.max_action);
        let min_action = action_to_tensor(dev, &self.min_action);

        let action =
            min_action.clone() + (max_action - min_action) * (action.clamp(-1., 1.) + 1.) / 2.;

        tensor_to_action(&action)
    }

    fn deterministic_action(&self, point: &Point) -> Action {
        self.raw_action(point, |_, mean| mean)
    }
}

impl RemyPolicy for RemyrDna {
    fn action(&self, point: &Point, _time: Time) -> Option<Action> {
        Some(self.deterministic_action(point))
    }
}
