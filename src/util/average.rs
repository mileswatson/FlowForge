use std::{
    fmt::Debug,
    ops::{Add, Div},
};

use crate::quantities::Float;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoItems;

pub trait Average: Sized {
    type Aggregator;
    type Output;

    fn new_aggregator() -> Self::Aggregator;

    fn aggregate(aggregator: Self::Aggregator, next: Self) -> Self::Aggregator;

    fn average(aggregator: Self::Aggregator) -> Self::Output;
}

impl<T> Average for T
where
    T: Add<T, Output = T> + Div<Float, Output = T>,
{
    type Aggregator = Option<(T, usize)>;
    type Output = Result<T, NoItems>;

    fn new_aggregator() -> Self::Aggregator {
        None
    }

    fn aggregate(aggregator: Self::Aggregator, next: Self) -> Self::Aggregator {
        match aggregator {
            Some((total, count)) => Some((total + next, count + 1)),
            None => Some((next, 1)),
        }
    }

    fn average(aggregator: Self::Aggregator) -> Self::Output {
        #[allow(clippy::cast_precision_loss)]
        match aggregator {
            Some((total, count)) => Ok(total / count as Float),
            None => Err(NoItems),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AveragePair<T, U>(pub T, pub U);

impl<T, U> AveragePair<T, U> {
    pub fn new((t, u): (T, U)) -> Self {
        AveragePair(t, u)
    }
}

impl<T, U> Average for AveragePair<T, U>
where
    T: Average,
    U: Average,
{
    type Aggregator = (T::Aggregator, U::Aggregator);
    type Output = (T::Output, U::Output);

    fn new_aggregator() -> Self::Aggregator {
        (T::new_aggregator(), U::new_aggregator())
    }

    fn aggregate(current: Self::Aggregator, next: Self) -> Self::Aggregator {
        (
            T::aggregate(current.0, next.0),
            U::aggregate(current.1, next.1),
        )
    }

    fn average(aggregate: Self::Aggregator) -> Self::Output {
        (T::average(aggregate.0), U::average(aggregate.1))
    }
}

pub trait SameEmptiness<T, U> {
    fn assert_same_emptiness(self) -> Result<(T, U), NoItems>;
}

impl<T, U> SameEmptiness<T, U> for (Result<T, NoItems>, Result<U, NoItems>)
where
    T: Debug,
    U: Debug,
{
    fn assert_same_emptiness(self) -> Result<(T, U), NoItems> {
        match (self.0, self.1) {
            (Ok(t), Ok(u)) => Ok((t, u)),
            (Err(NoItems), Err(NoItems)) => Err(NoItems),
            x => panic!("Averages have different emptiness: {x:?}"),
        }
    }
}

#[derive(Debug)]
pub struct AverageIfSome<T>(Option<T>)
where
    T: Average;

impl<T> AverageIfSome<T>
where
    T: Average,
{
    pub const fn new(value: Option<T>) -> AverageIfSome<T> {
        AverageIfSome(value)
    }

    pub const fn some(value: T) -> AverageIfSome<T> {
        AverageIfSome(Some(value))
    }
}

impl<T> Average for AverageIfSome<T>
where
    T: Average,
{
    type Aggregator = T::Aggregator;
    type Output = T::Output;

    fn new_aggregator() -> Self::Aggregator {
        T::new_aggregator()
    }

    fn aggregate(current: Self::Aggregator, next: Self) -> Self::Aggregator {
        match next.0 {
            Some(next) => T::aggregate(current, next),
            None => current,
        }
    }

    fn average(aggregator: Self::Aggregator) -> Self::Output {
        T::average(aggregator)
    }
}

pub trait IterAverage<T>
where
    T: Average,
{
    fn average(self) -> T::Output;
}

impl<T, I> IterAverage<T> for I
where
    I: IntoIterator<Item = T>,
    T: Average,
{
    fn average(self) -> <T as Average>::Output {
        T::average(self.into_iter().fold(T::new_aggregator(), T::aggregate))
    }
}

#[cfg(test)]
mod tests {
    use std::iter::once;

    use crate::{
        quantities::Float,
        util::average::{AverageIfSome, IterAverage, NoItems},
    };

    use super::{AveragePair, SameEmptiness};

    #[test]
    fn empty() {
        assert_eq!(Vec::<Float>::new().average(), Err(NoItems));
    }

    #[test]
    fn average_pair() {
        let average = (0..5)
            .map(Float::from)
            .zip(
                (5..9)
                    .map(Float::from)
                    .map(AverageIfSome::some)
                    .chain(once(AverageIfSome::new(None))),
            )
            .map(AveragePair::new)
            .average();
        assert_eq!((average.0.unwrap(), average.1.unwrap()), (2., 6.5));
    }

    #[test]
    fn same_emptiness() {
        assert_eq!(
            (0..0)
                .map(Float::from)
                .zip((1..1).map(Float::from))
                .map(AveragePair::new)
                .average()
                .assert_same_emptiness(),
            Err(NoItems)
        );
        assert_eq!(
            (0..2)
                .map(Float::from)
                .zip((2..4).map(Float::from))
                .map(AveragePair::new)
                .average()
                .assert_same_emptiness(),
            Ok((0.5, 2.5))
        );
    }

    #[test]
    #[should_panic = "different emptiness"]
    fn different_emptiness1() {
        let _ = (0..2)
            .map(|x| AveragePair(Float::from(x), AverageIfSome::<Float>::new(None)))
            .average()
            .assert_same_emptiness();
    }

    #[test]
    #[should_panic = "different emptiness"]
    fn different_emptiness2() {
        let _ = (0..2)
            .map(|x| AveragePair(AverageIfSome::<Float>::new(None), Float::from(x)))
            .average()
            .assert_same_emptiness();
    }
}
