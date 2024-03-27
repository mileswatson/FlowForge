use std::fmt::{self, Debug, Formatter};

use dfdx::{
    nn::Module,
    shapes::Const,
    tensor::{AsArray, Cpu, Tensor, TensorFrom, Tensorlike},
};

use crate::{core::rand::Wrapper, quantities::TimeSpan};

use self::{
    dna::RemyrDna,
    net::{AsPolicyNetRef, ACTION, STATE},
};

use super::remy::{action::Action, point::Point, rule_tree::RuleTree};

pub mod dna;
pub mod net;

pub struct RuleNetwork<T>(T);

impl<T> Debug for RuleNetwork<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RuleNetwork").finish()
    }
}

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

impl<P> RuleTree for RemyrDna<P>
where
    P: AsPolicyNetRef,
{
    type Action<'b> = Action where Self: 'b;

    fn action(&self, point: &Point) -> Option<Action> {
        let dev = self.policy.as_policy_net_ref().0 .0.weight.dev();
        let point = point_to_tensor(dev, point);
        let min_point = point_to_tensor(dev, &self.min_point);
        let max_point = point_to_tensor(dev, &self.max_point);
        let input = ((point - min_point.clone()) / (max_point - min_point)).clamp(0., 1.) * 2. - 1.;
        let (mean, _) = self.policy.as_policy_net_ref().forward(input);
        let max_action = action_to_tensor(dev, &self.max_action);
        let min_action = action_to_tensor(dev, &self.min_action);

        let action =
            min_action.clone() + (max_action - min_action) * (mean.clamp(-1., 1.) + 1.) / 2.;

        Some(tensor_to_action(&action))
    }
}
