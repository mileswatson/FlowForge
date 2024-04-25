use derive_more::{From, TryInto};
use serde::{Deserialize, Serialize};

use crate::{
    components::{packet::Packet, senders::lossy::LossySenderEffect, toggler::Toggle},
    util::{never::Never, WithLifetime},
};

use self::{delay_multiplier::DelayMultiplierTrainer, remy::RemyTrainer, remyr::RemyrTrainer};

pub mod delay_multiplier;
pub mod genetic;
pub mod remy;
pub mod remyr;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TrainerConfig {
    Remy(RemyTrainer),
    Remyr(RemyrTrainer),
    DelayMultiplier(DelayMultiplierTrainer),
}

#[derive(From, TryInto)]
pub enum DefaultEffect<'sim> {
    LossySender(LossySenderEffect<'sim, DefaultEffect<'sim>>),
    Packet(Packet<'sim, DefaultEffect<'sim>>),
    Toggle(Toggle),
    Never(Never),
}

impl<'sim> WithLifetime for DefaultEffect<'sim> {
    type Type<'a> = DefaultEffect<'a>;
}
