use std::{
    iter::successors,
    ops::{Add, Mul},
};

use itertools::Itertools;
use protobuf::MessageField;
use serde::{Deserialize, Serialize};

use crate::time::Float;

use super::{
    autogen::remy_dna::{MemoryRange, Whisker},
    point::Point,
    RemyConfig,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub window_multiplier: Float,
    pub window_increment: i32,
    pub intersend_ms: Float,
}

fn changes<T>(initial_change: T, max_change: T, multiplier: i32) -> impl Iterator<Item = T> + Clone
where
    T: From<i32> + Mul<T, Output = T> + PartialOrd + Copy + 'static,
{
    successors(Some(initial_change), move |x| {
        Some(T::from(multiplier) * *x)
    })
    .take_while(move |x| x <= &max_change)
    .flat_map(|x| [x, T::from(-1) * x])
}

impl Action {
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
        changes(
            initial_action_change.window_multiplier,
            max_action_change.window_multiplier,
            *action_change_multiplier,
        )
        .cartesian_product(changes(
            initial_action_change.window_increment,
            max_action_change.window_increment,
            *action_change_multiplier,
        ))
        .cartesian_product(changes(
            initial_action_change.intersend_ms,
            max_action_change.intersend_ms,
            *action_change_multiplier,
        ))
        .map(
            move |((window_multiplier, window_increment), intersend_ms)| {
                &cloned
                    + &Action {
                        window_multiplier,
                        window_increment,
                        intersend_ms,
                    }
            },
        )
        .filter(move |x| {
            min_action.window_multiplier <= x.window_multiplier
                && x.window_multiplier <= max_action.window_multiplier
                && min_action.window_increment <= x.window_increment
                && x.window_increment <= max_action.window_increment
                && min_action.intersend_ms <= x.intersend_ms
                && x.intersend_ms <= max_action.intersend_ms
        })
    }

    #[must_use]
    pub fn from_whisker(whisker: &MessageField<Whisker>) -> Action {
        Action {
            window_multiplier: whisker.window_multiple(),
            window_increment: whisker.window_increment(),
            intersend_ms: whisker.intersend(),
        }
    }
}

impl Whisker {
    pub fn create(value: &Action, min: &Point, max: &Point) -> Self {
        let mut memory_range = MemoryRange::new();
        memory_range.lower = MessageField::some(min.to_memory());
        memory_range.upper = MessageField::some(max.to_memory());
        let mut whisker = Whisker::new();
        whisker.set_intersend(value.intersend_ms);
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
            intersend_ms: Float::from(self) * rhs.intersend_ms,
        }
    }
}

impl Add<&Action> for &Action {
    type Output = Action;

    fn add(self, rhs: &Action) -> Self::Output {
        Action {
            window_multiplier: self.window_multiplier + rhs.window_multiplier,
            window_increment: self.window_increment + rhs.window_increment,
            intersend_ms: self.intersend_ms + rhs.intersend_ms,
        }
    }
}
