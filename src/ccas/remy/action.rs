use std::{
    fmt::{Debug, Display},
    ops::{Add, Mul},
};

use format_num::format_num;
use protobuf::MessageField;
use serde::{Deserialize, Serialize};

use crate::quantities::{milliseconds, seconds, Float, TimeSpan};

use super::{
    autogen::remy_dna::{MemoryRange, Whisker},
    point::Point,
};

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct Action<const TESTING: bool = false> {
    pub window_multiplier: Float,
    pub window_increment: i32,
    pub intersend_delay: TimeSpan,
}

impl<const TESTING: bool> AsRef<Action<TESTING>> for Action<TESTING> {
    fn as_ref(&self) -> &Action<TESTING> {
        self
    }
}

impl<const TESTING: bool> Display for Action<TESTING> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Action {{ window_multiplier: {}, window_increment: {}, intersend_delay: {} }}",
            &format_num!(".3f", self.window_multiplier),
            &self.window_increment,
            &self.intersend_delay,
        )
    }
}

impl<const TESTING: bool> Action<TESTING> {
    #[must_use]
    pub fn from_whisker(whisker: &MessageField<Whisker>) -> Action<TESTING> {
        Action {
            window_multiplier: whisker.window_multiple(),
            window_increment: whisker.window_increment(),
            intersend_delay: if TESTING {
                seconds(whisker.intersend())
            } else {
                milliseconds(whisker.intersend())
            },
        }
    }

    #[must_use]
    pub fn apply_to(&self, window: u32) -> u32 {
        #[allow(clippy::cast_sign_loss)]
        return ((f64::from(window) * self.window_multiplier) as i32 + self.window_increment)
            .clamp(0, 1_000_000) as u32;
    }
}

impl Whisker {
    pub fn create<const TESTING: bool>(
        value: &Action<TESTING>,
        min: &Point<TESTING>,
        max: &Point<TESTING>,
    ) -> Self {
        let mut memory_range = MemoryRange::new();
        memory_range.lower = MessageField::some(min.to_memory());
        memory_range.upper = MessageField::some(max.to_memory());
        let mut whisker = Whisker::new();
        whisker.set_intersend(if TESTING {
            value.intersend_delay.seconds()
        } else {
            value.intersend_delay.milliseconds()
        });
        whisker.set_window_increment(value.window_increment);
        whisker.set_window_multiple(value.window_multiplier);
        whisker.domain = MessageField::some(memory_range);
        whisker
    }
}

impl Mul<&Action> for i32 {
    type Output = Action;

    fn mul(self, rhs: &Action) -> Self::Output {
        Action {
            window_multiplier: Float::from(self) * rhs.window_multiplier,
            window_increment: self * rhs.window_increment,
            intersend_delay: Float::from(self) * rhs.intersend_delay,
        }
    }
}

impl<const TESTING: bool> Add<&Action<TESTING>> for &Action<TESTING> {
    type Output = Action;

    fn add(self, rhs: &Action<TESTING>) -> Self::Output {
        Action {
            window_multiplier: self.window_multiplier + rhs.window_multiplier,
            window_increment: self.window_increment + rhs.window_increment,
            intersend_delay: self.intersend_delay + rhs.intersend_delay,
        }
    }
}
