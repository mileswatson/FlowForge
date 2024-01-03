use std::ops::{Add, Div};

use crate::time::Float;

pub struct IterEmpty;

pub fn average<I, T>(items: I) -> Result<T, IterEmpty>
where
    T: Average,
    I: IntoIterator<Item = T>,
    I::IntoIter: Clone,
{
    let mut iter = items.into_iter();
    iter.next()
        .map_or(Err(IterEmpty), |x| Ok(T::average(x, iter)))
}

pub trait Average: Clone + Sized {
    fn average<I>(first_item: Self, remaining_items: I) -> Self
    where
        I: IntoIterator<Item = Self>,
        I::IntoIter: Clone;
}

impl<T> Average for T
where
    T: Add<T, Output = T> + Div<Float, Output = T> + Clone,
{
    fn average<'a, I>(first_item: T, remaining_items: I) -> T
    where
        I: IntoIterator<Item = Self>,
    {
        let x = remaining_items
            .into_iter()
            .fold((first_item, 1), |(acc, count), x| (acc + x, count + 1));
        x.0.clone() / f64::from(x.1)
    }
}

#[derive(Clone)]
pub struct AveragePair<T, U>(pub T, pub U);

impl<T, U> AveragePair<T, U> {
    pub fn new((t, u): (T, U)) -> Self {
        AveragePair(t, u)
    }

    pub fn into_inner(self) -> (T, U) {
        (self.0, self.1)
    }
}

impl<T, U> Div<Float> for AveragePair<T, U>
where
    T: Div<Float, Output = T>,
    U: Div<Float, Output = U>,
{
    type Output = AveragePair<T, U>;

    fn div(self, rhs: Float) -> Self::Output {
        AveragePair(self.0 / rhs, self.1 / rhs)
    }
}

impl<T, U> Average for AveragePair<T, U>
where
    T: Average,
    U: Average,
{
    fn average<I>(first_item: Self, remaining_items: I) -> Self
    where
        I: IntoIterator<Item = Self>,
        I::IntoIter: Clone,
    {
        let (ts, us): (Vec<_>, Vec<_>) = remaining_items
            .into_iter()
            .map(AveragePair::into_inner)
            .unzip();
        AveragePair(T::average(first_item.0, ts), U::average(first_item.1, us))
    }
}
