use std::{
    fmt::Debug,
    iter::successors,
    ops::{Add, Mul},
};

use format_num::format_num;
use itertools::Itertools;
use protobuf::MessageField;
use serde::{Deserialize, Serialize};

use crate::quantities::{Float, TimeSpan, seconds, milliseconds};

use super::{
    autogen::remy_dna::{MemoryRange, Whisker},
    point::Point,
    RemyConfig,
};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Action<const TESTING: bool = false> {
    pub window_multiplier: Float,
    pub window_increment: i32,
    pub intersend_delay: TimeSpan,
}

impl<const TESTING: bool> Debug for Action<TESTING> {
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

fn changes<T, U>(
    initial_change: T,
    max_change: T,
    multiplier: i32,
) -> impl Iterator<Item = T> + Clone
where
    T: PartialOrd + Copy + 'static,
    U: From<i32> + Mul<T, Output = T>,
{
    successors(Some(initial_change), move |x| {
        Some(U::from(multiplier) * *x)
    })
    .take_while(move |x| x <= &max_change)
    .flat_map(|x| [x, U::from(-1) * x])
}

impl<const TESTING: bool> Action<TESTING> {
    pub fn possible_improvements<'a>(
        &self,
        RemyConfig {
            initial_action_change,
            max_action_change,
            action_change_multiplier,
            min_action,
            max_action,
            ..
        }: &'a RemyConfig,
    ) -> impl Iterator<Item = Action> + 'a {
        let cloned = self.clone();
        changes::<Float, Float>(
            initial_action_change.window_multiplier,
            max_action_change.window_multiplier,
            *action_change_multiplier,
        )
        .cartesian_product(changes::<i32, i32>(
            initial_action_change.window_increment,
            max_action_change.window_increment,
            *action_change_multiplier,
        ))
        .cartesian_product(changes::<TimeSpan, Float>(
            initial_action_change.intersend_delay,
            max_action_change.intersend_delay,
            *action_change_multiplier,
        ))
        .map(
            move |((window_multiplier, window_increment), intersend_ms)| {
                &cloned
                    + &Action::<TESTING> {
                        window_multiplier,
                        window_increment,
                        intersend_delay: intersend_ms,
                    }
            },
        )
        .filter(move |x| {
            min_action.window_multiplier <= x.window_multiplier
                && x.window_multiplier <= max_action.window_multiplier
                && min_action.window_increment <= x.window_increment
                && x.window_increment <= max_action.window_increment
                && min_action.intersend_delay <= x.intersend_delay
                && x.intersend_delay <= max_action.intersend_delay
        })
    }

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
