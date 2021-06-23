#![cfg_attr(not(test), no_std)]

//! A collection of useful `nb` extensions.

use core::{
    future::Future,
    task::{Context, Poll},
};
use futures_util::Stream;

#[cfg(test)]
mod tests;

pub trait NbResultExt<T, E> {
    fn filter<P: FnOnce(&T) -> bool>(self, pred: P) -> Self;

    fn filter_map<U, P: FnOnce(T) -> Option<U>>(self, pred: P) -> nb::Result<U, E>;

    fn expect_ok(self, msg: &str) -> Option<T>;

    /// Converts the `nb::Result` value into the corresponding `Poll` one.
    fn into_poll(self, ctx: &mut Context<'_>) -> Poll<Result<T, E>>;
}

impl<T, E> NbResultExt<T, E> for nb::Result<T, E> {
    fn filter<P: FnOnce(&T) -> bool>(self, pred: P) -> Self {
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

    fn filter_map<U, P: FnOnce(T) -> Option<U>>(self, pred: P) -> nb::Result<U, E> {
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

    #[track_caller]
    fn expect_ok(self, msg: &str) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(nb::Error::WouldBlock) => None,

            _ => panic!("{}", msg),
        }
    }

    /// Converts the `nb::Result` value into the corresponding `Poll` one. 
    /// For the `nb::Err::WouldBlock` value it calls a waker.
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