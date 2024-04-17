use derive_more::{From, TryInto};
use serde::{Deserialize, Serialize};

use crate::{
    core::never::Never,
    components::{
        senders::window::{
            LossyInternalControllerEffect, LossyInternalSenderEffect, LossySenderEffect,
        },
        toggler::Toggle,
        EffectTypeGenerator, Packet,
    },
};

use self::{delay_multiplier::DelayMultiplierConfig, remy::RemyConfig, remyr::RemyrConfig};

pub mod delay_multiplier;
pub mod genetic;
pub mod remy;
pub mod remyr;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TrainerConfig {
    Remy(RemyConfig),
    Remyr(RemyrConfig),
    DelayMultiplier(DelayMultiplierConfig),
}

#[derive(From, TryInto)]
pub enum DefaultEffect<'sim> {
    Packet(Packet<'sim, DefaultEffect<'sim>>),
    LossySenderEffect(LossySenderEffect<'sim, DefaultEffect<'sim>>),
    LossyInternalControllerEffect(LossyInternalControllerEffect),
    LossyInternalSenderEffect(LossyInternalSenderEffect<'sim, DefaultEffect<'sim>>),
    Toggle(Toggle),
    Never(Never),
}

impl<'sim> EffectTypeGenerator for DefaultEffect<'sim> {
    type Type<'a> = DefaultEffect<'a>;
}
