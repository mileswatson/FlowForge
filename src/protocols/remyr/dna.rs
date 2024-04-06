use std::{
    fmt::{self, Debug, Formatter},
    fs::File,
    io::{Read, Write},
};

use dfdx::{
    nn::{BuildModuleExt, LoadSafeTensors, SaveSafeTensors},
    tensor::Cpu,
};
use serde::{Deserialize, Serialize};

use crate::{
    protocols::remy::{action::Action, point::Point},
    Dna,
};

use super::net::{AsPolicyNetRef, HiddenLayers, PolicyNet, PolicyNetwork};

pub trait SerializeTensors {
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(&mut self, buf: &[u8]);
}

impl<T: SaveSafeTensors + LoadSafeTensors> SerializeTensors for T {
    fn serialize(&self) -> Vec<u8> {
        let temp_dir = tempfile::tempdir().unwrap();
        let filepath = temp_dir.path().join("temp");
        self.save_safetensors(&filepath).unwrap();
        let mut file = File::open(filepath).unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        buf
    }

    fn deserialize(&mut self, buf: &[u8]) {
        let temp_dir = tempfile::tempdir().unwrap();
        let filepath = temp_dir.path().join("temp");
        {
            let mut file = File::create(&filepath).unwrap();
            file.write_all(buf).unwrap();
        }
        self.load_safetensors(filepath).unwrap();
    }
}

#[derive(Serialize, Deserialize)]
struct _RemyrDna {
    min_point: Point,
    max_point: Point,
    min_action: Action,
    max_action: Action,
    hidden_layers: HiddenLayers,
    policy: Vec<u8>,
}

pub struct RemyrDna {
    pub min_point: Point,
    pub max_point: Point,
    pub min_action: Action,
    pub max_action: Action,
    pub policy: PolicyNetwork<Cpu>,
}

impl Debug for RemyrDna {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemyrDna")
            .field("min_point", &self.min_point)
            .field("max_point", &self.max_point)
            .field("min_action", &self.min_action)
            .field("max_action", &self.max_action)
            .field("policy", &self.policy.as_policy_net_ref().hidden_layers())
            .finish()
    }
}

impl Serialize for RemyrDna {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        _RemyrDna {
            min_point: self.min_point.clone(),
            max_point: self.max_point.clone(),
            min_action: self.min_action.clone(),
            max_action: self.max_action.clone(),
            hidden_layers: self.policy.as_policy_net_ref().hidden_layers(),
            policy: self.policy.serialize(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RemyrDna {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _RemyrDna {
            min_point,
            max_point,
            min_action,
            max_action,
            hidden_layers,
            policy,
        } = _RemyrDna::deserialize(deserializer)?;
        let cpu = Cpu::default();
        let mut loaded_policy = cpu.build_module(hidden_layers.policy_arch());
        loaded_policy.deserialize(&policy);
        Ok(RemyrDna {
            min_point,
            max_point,
            min_action,
            max_action,
            policy: loaded_policy,
        })
    }
}

impl Dna for RemyrDna {
    const NAME: &'static str = "remyr";

    fn serialize(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    fn deserialize(buf: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(buf)?)
    }
}

#[cfg(test)]
mod tests {
    use dfdx::{
        nn::{BuildModuleExt, Module},
        tensor::{AsArray, Cpu, TensorFrom},
    };

    use crate::protocols::remyr::{dna::SerializeTensors, net::HiddenLayers};

    #[test]
    fn serialize_deserialize() {
        let cpu = Cpu::default();
        let critic = cpu.build_module::<f32>(HiddenLayers(32, 16).critic_arch());
        let mut new_critic = cpu.build_module::<f32>(HiddenLayers(32, 16).critic_arch());
        let x = cpu.tensor([[1., 2., 3., 4.]]);
        assert!(critic.forward(x.clone()).array() != new_critic.forward(x.clone()).array());
        new_critic.deserialize(&critic.serialize());
        assert!(critic.forward(x.clone()).array() == new_critic.forward(x).array());
        dbg!(new_critic);
    }
}
