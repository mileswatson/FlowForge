use std::ops::{Add, Sub};

use serde::{Serialize, Deserialize};

#[derive(PartialEq, Eq, PartialOrd, Clone, Copy, Serialize, Deserialize, Debug)]
pub struct Information(u64);

impl Information {
    pub const ZERO: Information = Information(0);

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.bytes() * 8
    }

    #[must_use]
    pub const fn bytes(self) -> u64 {
        self.0
    }
}

#[must_use]
pub const fn bytes(value: u64) -> Information {
    Information(value)
}

#[must_use]
pub const fn packets(value: u64) -> Information {
    bytes(1400 * value)
}

impl Add<Information> for Information {
    type Output = Information;

    fn add(self, rhs: Information) -> Self::Output {
        Information(self.0 + rhs.0)
    }
}

impl Sub<Information> for Information {
    type Output = Information;

    fn sub(self, rhs: Information) -> Self::Output {
        Information(self.0 - rhs.0)
    }
}
