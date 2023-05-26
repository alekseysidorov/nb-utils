//! A collection of useful `nb` extensions.

#![cfg_attr(not(any(test, feature = "std")), no_std)]

#[cfg(feature = "std")]
pub use crate::std::IntoNbResult;
use core::{
    fmt::Debug,
    future::Future,
    task::{Context, Poll},
};
use futures_util::Stream;

#[cfg(feature = "std")]
mod std;
#[cfg(test)]
mod tests;

/// Extension trait for [`nb::Result`] type.
pub trait NbResultExt<T, E> {
    /// Returns `Ok` if the given predicate applied to the `Ok` value returns true;
    /// otherwise returns [`nb::Error::WouldBlock`].
    fn wait<P: FnOnce(&T) -> bool>(self, pred: P) -> Self;
    /// Returns `Ok` if the given predicate applied to the `Ok` value returns `Some`;
    /// otherwise returns [`nb::Error::WouldBlock`].
    fn wait_map<U, P: FnOnce(T) -> Option<U>>(self, pred: P) -> nb::Result<U, E>;
    /// Invokes given closure in the result is `Ok` and do nothing if the result
    /// is [`nb::Error::WouldBlock`].
    fn if_ready<F, U>(self, then: F) -> Result<(), E>
    where
        F: FnOnce(T) -> Result<(), U>,
        E: From<U>;
    /// Unlike [`core::result::Result::expect`] returns `None`
    /// if `Err` is [`nb::Error::WouldBlock`].
    fn expect_ok(self, msg: &str) -> Option<T>
    where
        E: Debug;
    /// Converts the `nb::Result` value into the corresponding `Poll` one.
    /// For the [`nb::Error::WouldBlock`] value it calls a waker.
    fn into_poll(self, ctx: &mut Context<'_>) -> Poll<Result<T, E>>;
    /// Returns true if the result is [`nb::Error::WouldBlock`].
    fn is_would_block(&self) -> bool;
}

impl<T, E> NbResultExt<T, E> for nb::Result<T, E> {
    fn wait<P: FnOnce(&T) -> bool>(self, pred: P) -> Self {
        match self {
            Ok(value) => {
                if pred(&value) {
                    Ok(value)
                } else {
                    Err(nb::Error::WouldBlock)
                }
            }
            other => other,
        }
    }

    fn wait_map<U, P: FnOnce(T) -> Option<U>>(self, pred: P) -> nb::Result<U, E> {
        match self {
            Ok(value) => {
                if let Some(value) = pred(value) {
                    Ok(value)
                } else {
                    Err(nb::Error::WouldBlock)
                }
            }
            Err(nb::Error::Other(other)) => Err(nb::Error::Other(other)),
            Err(nb::Error::WouldBlock) => Err(nb::Error::WouldBlock),
        }
    }

    fn if_ready<F, U>(self, then: F) -> Result<(), E>
    where
        F: FnOnce(T) -> Result<(), U>,
        E: From<U>,
    {
        match self {
            Err(nb::Error::Other(e)) => Err(e),
            Err(nb::Error::WouldBlock) => Ok(()),
            Ok(value) => then(value).map_err(E::from),
        }
    }

    #[track_caller]
    fn expect_ok(self, msg: &str) -> Option<T>
    where
        E: Debug,
    {
        match self {
            Ok(value) => Some(value),
            Err(nb::Error::WouldBlock) => None,
            Err(nb::Error::Other(error)) => panic!("{msg} {error:?}"),
        }
    }

    fn into_poll(self, ctx: &mut Context<'_>) -> Poll<Result<T, E>> {
        match self {
            Ok(output) => Poll::Ready(Ok(output)),
            Err(nb::Error::Other(err)) => Poll::Ready(Err(err)),
            Err(nb::Error::WouldBlock) => {
                ctx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }

    fn is_would_block(&self) -> bool {
        matches!(self, Err(nb::Error::WouldBlock))
    }
}

/// Convert a function that returns `nb::Result<T, E>` into a valid but inefficient future. The future will
/// resolve only when the function returns `Ok(T)` or `Err(nb::Error::Other).
pub fn poll_nb_future<T, E, F>(mut poll_fn: F) -> impl Future<Output = Result<T, E>>
where
    F: FnMut() -> nb::Result<T, E>,
{
    futures_util::future::poll_fn(move |ctx| poll_fn().into_poll(ctx))
}

/// Convert a function that returns `nb::Result<T, E>` into a valid but inefficient infinite stream.
/// The next stream item will resolve only when the function returns `Ok(T)` or `Err(nb::Error::Other).
pub fn poll_nb_stream<T, E, F>(mut poll_fn: F) -> impl Stream<Item = Result<T, E>>
where
    F: FnMut() -> nb::Result<T, E> + Unpin,
{
    futures_util::stream::poll_fn(move |ctx| poll_fn().into_poll(ctx).map(Some))
}

/// Creates future which always returns `Poll::Pending` at the first `poll` call to transfer the
/// control flow to the executor.
pub fn yield_executor() -> impl Future<Output = ()> {
    let mut yielded = false;
    futures_util::future::poll_fn(move |ctx| {
        if !yielded {
            yielded = true;
            ctx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
}
