use std::{
    fmt::Debug,
    ops::{Add, Div},
};

use crate::time::Float;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoItems;

pub trait Average: Sized {
    type Output;

    fn average<I>(items: I) -> Self::Output
    where
        I: IntoIterator<Item = Self>;
}

impl<T> Average for T
where
    T: Add<T, Output = T> + Div<Float, Output = T>,
{
    type Output = Result<T, NoItems>;

    fn average<'a, I>(items: I) -> Result<T, NoItems>
    where
        I: IntoIterator<Item = Self>,
    {
        let mut iter = items.into_iter();
        match iter.next() {
            Some(first_item) => {
                let x = iter.fold((first_item, 1usize), |(acc, count), x| (acc + x, count + 1));
                #[allow(clippy::cast_precision_loss)]
                return Ok(x.0 / x.1 as Float);
            }
            None => Err(NoItems),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AverageSeparately<T, U>(pub T, pub U);

impl<T, U> AverageSeparately<T, U> {
    pub fn new((t, u): (T, U)) -> Self {
        AverageSeparately(t, u)
    }

    fn into_inner(self) -> (T, U) {
        (self.0, self.1)
    }
}

impl<T, U> Average for AverageSeparately<T, U>
where
    T: Average,
    U: Average,
{
    type Output = (T::Output, U::Output);

    fn average<I>(items: I) -> Self::Output
    where
        I: IntoIterator<Item = Self>,
    {
        let (ts, us): (Vec<_>, Vec<_>) =
            items.into_iter().map(AverageSeparately::into_inner).unzip();
        (T::average(ts), U::average(us))
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

    pub fn into_inner(self) -> Option<T> {
        self.0
    }
}

impl<T> Average for AverageIfSome<T>
where
    T: Average,
{
    type Output = T::Output;

    fn average<I>(items: I) -> Self::Output
    where
        I: IntoIterator<Item = Self>,
    {
        items
            .into_iter()
            .filter_map(AverageIfSome::into_inner)
            .average()
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
        T::average(self)
    }
}

#[cfg(test)]
mod test {
    use std::iter::once;

    use crate::{
        average::{AverageIfSome, IterAverage, NoItems},
        time::Float,
    };

    use super::{AverageSeparately, SameEmptiness};

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
            .map(AverageSeparately::new)
            .average();
        assert_eq!((average.0.unwrap(), average.1.unwrap()), (2., 6.5));
    }

    #[test]
    fn same_emptiness() {
        assert_eq!(
            (0..0)
                .map(Float::from)
                .zip((1..1).map(Float::from))
                .map(AverageSeparately::new)
                .average()
                .assert_same_emptiness(),
            Err(NoItems)
        );
        assert_eq!(
            (0..2)
                .map(Float::from)
                .zip((2..4).map(Float::from))
                .map(AverageSeparately::new)
                .average()
                .assert_same_emptiness(),
            Ok((0.5, 2.5))
        );
    }

    #[test]
    #[should_panic = "different emptiness"]
    fn different_emptiness1() {
        let _ = (0..2)
            .map(|x| AverageSeparately(Float::from(x), AverageIfSome::<Float>::new(None)))
            .average()
            .assert_same_emptiness();
    }

    #[test]
    #[should_panic = "different emptiness"]
    fn different_emptiness2() {
        let _ = (0..2)
            .map(|x| AverageSeparately(AverageIfSome::<Float>::new(None), Float::from(x)))
            .average()
            .assert_same_emptiness();
    }
}
