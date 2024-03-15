//! Utilities for working with [Extend].
use std::iter::Extend;

/// Calls `f` for each `Item`.
///
/// ```
/// # use om_wikiparser::extend;
/// let mut count = 0;
///
/// extend::from_fn(|_| count += 1).extend(std::iter::zip(
///     [1, 2, 3, 4],
///     ['a', 'b', 'c']));
/// assert_eq!(count, 3);
/// ```
pub fn from_fn<Item, F: FnMut(Item)>(f: F) -> FromFn<F> {
    FromFn(f)
}

pub struct FromFn<F>(F);
impl<Item, F: FnMut(Item)> Extend<Item> for FromFn<F> {
    fn extend<T: IntoIterator<Item = Item>>(&mut self, iter: T) {
        for item in iter {
            self.0(item);
        }
    }
}

/// Iterates but drops each `Item`.
pub fn sink() -> Sink {
    Sink(())
}

pub struct Sink(());
impl<Item> Extend<Item> for Sink {
    fn extend<T: IntoIterator<Item = Item>>(&mut self, iter: T) {
        for _item in iter {}
    }
}
