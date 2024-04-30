use dfdx::prelude::*;
use serde::{Deserialize, Serialize};

use super::dna::SerializeTensors;

pub const OBSERVATION: usize = 3;
pub const GLOBAL_STATE: usize = 1;
pub const AGENT_SPECIFIC_GLOBAL_STATE: usize = OBSERVATION + GLOBAL_STATE;
pub const ACTION: usize = 3;

pub type PolicyArchitecture = (
    (LinearConfig<Const<OBSERVATION>, usize>, Tanh),
    (LinearConfig<usize, usize>, Tanh),
    (LinearConfig<usize, Const<ACTION>>, Tanh),
);

pub type CriticArchitecture = (
    (LinearConfig<Const<AGENT_SPECIFIC_GLOBAL_STATE>, usize>, FastGeLU),
    (LinearConfig<usize, usize>, FastGeLU),
    (LinearConfig<usize, Const<1>>,),
);

pub type PolicyNetwork<D = Cpu> = <PolicyArchitecture as BuildOnDevice<f32, D>>::Built;

pub type CriticNetwork<D> = <CriticArchitecture as BuildOnDevice<f32, D>>::Built;

pub trait AsPolicyNetRef {
    fn as_policy_net_ref(&self) -> &PolicyNetwork<Cpu>;
}

impl AsPolicyNetRef for PolicyNetwork<Cpu> {
    fn as_policy_net_ref(&self) -> &PolicyNetwork<Cpu> {
        self
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct HiddenLayers(pub usize, pub usize);

impl HiddenLayers {
    pub fn new<D>(policy: &PolicyNetwork<D>) -> HiddenLayers
    where
        D: Device<f32>,
    {
        let (o, i) = policy.1 .0.weight.shape();
        HiddenLayers(*i, *o)
    }

    #[must_use]
    pub fn policy_arch(self) -> PolicyArchitecture {
        (
            (LinearConfig::new(Const::<OBSERVATION>, self.0), Tanh),
            (LinearConfig::new(self.0, self.1), Tanh),
            (LinearConfig::new(self.1, Const::<ACTION>), Tanh),
        )
    }

    #[must_use]
    pub fn critic_arch(self) -> CriticArchitecture {
        (
            (LinearConfig::new(Const::<AGENT_SPECIFIC_GLOBAL_STATE>, self.0), FastGeLU),
            (LinearConfig::new(self.0, self.1), FastGeLU),
            (LinearConfig::new(self.1, Const::<1>),),
        )
    }
}

pub trait CopyToDevice<D, M>
where
    D: Device<f32>,
    M: Device<f32>,
{
    type Architecture: BuildOnDevice<f32, D> + BuildOnDevice<f32, M>;

    fn copy_to(&self, device: &M) -> <Self::Architecture as BuildOnDevice<f32, M>>::Built;
}

impl<D, M> CopyToDevice<D, M> for PolicyNetwork<D>
where
    D: Device<f32>,
    M: Device<f32>,
{
    type Architecture = PolicyArchitecture;

    fn copy_to(&self, device: &M) -> <Self::Architecture as BuildOnDevice<f32, M>>::Built {
        let mut new = device.build_module(self.hidden_layers().policy_arch());
        new.deserialize(&self.serialize());
        new
    }
}

pub trait PolicyNet<D> {
    fn device(&self) -> &D;

    fn hidden_layers(&self) -> HiddenLayers;
}

impl<D> PolicyNet<D> for PolicyNetwork<D>
where
    D: Device<f32>,
{
    fn device(&self) -> &D {
        self.0 .0.weight.dev()
    }

    fn hidden_layers(&self) -> HiddenLayers {
        HiddenLayers::new(self)
    }
}

#[cfg(test)]
mod tests {
    use dfdx::{nn::BuildModuleExt, tensor::Cpu};

    use crate::ccas::remyr::dna::SerializeTensors;

    use super::HiddenLayers;

    #[test]
    fn determinism() {
        let dev1 = Cpu::default();
        let dev2 = Cpu::default();
        let n1 = dev1.build_module::<f32>(HiddenLayers(32, 32).policy_arch());
        let n2 = dev2.build_module::<f32>(HiddenLayers(32, 32).policy_arch());
        assert_eq!(n1.serialize(), n2.serialize());
        insta::assert_yaml_snapshot!(n1.serialize());
    }
}
