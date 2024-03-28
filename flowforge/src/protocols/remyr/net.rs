use dfdx::prelude::*;
use serde::{Deserialize, Serialize};

use super::dna::SerializeTensors;

pub const STATE: usize = 3;
pub const ACTION: usize = 3;

pub type PolicyArchitecture = (
    (LinearConfig<Const<STATE>, usize>, Tanh),
    (LinearConfig<usize, usize>, Tanh),
    SplitInto<(
        (LinearConfig<usize, Const<ACTION>>, Tanh),
        (LinearConfig<usize, Const<ACTION>>, Sigmoid),
    )>,
);

pub type CriticArchitecture = (
    (LinearConfig<Const<STATE>, usize>, FastGeLU),
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
            (LinearConfig::new(Const::<STATE>, self.0), Tanh),
            (LinearConfig::new(self.0, self.1), Tanh),
            SplitInto((
                (LinearConfig::new(self.1, Const::<ACTION>), Tanh),
                (LinearConfig::new(self.1, Const::<ACTION>), Sigmoid),
            )),
        )
    }

    #[must_use]
    pub fn critic_arch(self) -> CriticArchitecture {
        (
            (LinearConfig::new(Const::<STATE>, self.0), FastGeLU),
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
