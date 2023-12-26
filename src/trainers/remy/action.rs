use std::ops::{Add, Mul};

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

impl Action {
    #[must_use]
    pub fn possible_improvements(
        &self,
        action: &Action,
        RemyConfig {
            initial_action_change,
            max_action_change,
            action_change_multiplier,
            min_action,
            max_action,
            ..
        }: &RemyConfig,
    ) -> Vec<Action> {
        let mut results = Vec::new();
        let valid_action = |x: &Action| {
            min_action.window_multiplier <= x.window_multiplier
                && x.window_multiplier <= max_action.window_multiplier
                && min_action.window_increment <= x.window_increment
                && x.window_increment <= max_action.window_increment
                && min_action.intersend_ms <= x.intersend_ms
                && x.intersend_ms <= max_action.intersend_ms
        };
        let mut window_multiplier = initial_action_change.window_multiplier;
        while window_multiplier.abs() <= max_action_change.window_multiplier {
            let mut window_increment = initial_action_change.window_increment;
            while window_increment.abs() <= max_action_change.window_increment {
                let mut intersend_ms = initial_action_change.intersend_ms;
                while intersend_ms.abs() <= max_action_change.intersend_ms {
                    let increment = Action {
                        window_multiplier,
                        window_increment,
                        intersend_ms,
                    };
                    for mul in [1, -1] {
                        let new_action = action + &(mul * &increment);
                        if valid_action(&new_action) {
                            results.push(new_action);
                        }
                    }

                    intersend_ms *= Float::from(*action_change_multiplier);
                }
                window_increment *= action_change_multiplier;
            }
            window_multiplier *= Float::from(*action_change_multiplier);
        }
        results
    }
}

impl Whisker {
    pub fn create(value: &Action, min: Point, max: Point) -> Self {
        let mut memory_range = MemoryRange::new();
        memory_range.lower = MessageField::some(min.into());
        memory_range.upper = MessageField::some(max.into());
        let mut whisker = Whisker::new();
        whisker.set_intersend(value.intersend_ms);
        whisker.set_window_increment(value.window_increment);
        whisker.set_window_multiple(value.window_multiplier);
        whisker.domain = MessageField::some(memory_range);
        whisker
    }
}

impl From<MessageField<Whisker>> for Action {
    fn from(value: MessageField<Whisker>) -> Self {
        Action {
            window_multiplier: value.window_multiple(),
            window_increment: value.window_increment(),
            intersend_ms: value.intersend(),
        }
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
