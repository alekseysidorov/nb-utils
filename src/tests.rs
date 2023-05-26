use futures_util::StreamExt;

use crate::{poll_nb_future, poll_nb_stream, yield_executor, NbResultExt};

struct MaybeBlock {
    attempts_per_item: usize,
    value: usize,

    remaining_attemts: usize,
}

impl Default for MaybeBlock {
    fn default() -> Self {
        Self {
            attempts_per_item: 5,
            value: 0,

            remaining_attemts: 1,
        }
    }
}

impl MaybeBlock {
    fn poll_me(&mut self) -> nb::Result<usize, ()> {
        if self.remaining_attemts == 0 {
            let value = self.value;

            self.value += 1;
            self.remaining_attemts = self.attempts_per_item;
            return Ok(value);
        }

        self.remaining_attemts -= 1;
        Err(nb::Error::WouldBlock)
    }
}

#[test]
fn test_maybe_block() {
    let mut block = MaybeBlock {
        value: 1,
        ..MaybeBlock::default()
    };

    let value = nb::block!(block.poll_me()).unwrap();
    assert_eq!(1, value);
}

#[test]
fn test_filter() {
    let mut block = MaybeBlock::default();

    let value = nb::block!(block.poll_me().wait(|value| *value == 5)).unwrap();
    assert_eq!(5, value);
}

#[test]
fn test_filter_map() {
    let mut block = MaybeBlock::default();

    let value =
        nb::block!(block
            .poll_me()
            .wait_map(|value| if value == 5 { Some("ready") } else { None }))
        .unwrap();
    assert_eq!("ready", value);
}

#[test]
fn test_poll_nb_future() {
    let mut block = MaybeBlock {
        value: 1,
        ..MaybeBlock::default()
    };

    let poll_me_async = poll_nb_future(|| block.poll_me());
    let value = spin_on::spin_on(poll_me_async).unwrap();

    assert_eq!(value, 1);
}

#[test]
fn test_poll_nb_stream() {
    let mut block = MaybeBlock {
        value: 1,
        ..MaybeBlock::default()
    };

    let mut poll_me_async = poll_nb_stream(move || block.poll_me());
    spin_on::spin_on(async {
        assert_eq!(poll_me_async.next().await, Some(Ok(1)));
        assert_eq!(poll_me_async.next().await, Some(Ok(2)));
    });
}

#[test]
fn test_yield() {
    spin_on::spin_on(async {
        yield_executor().await;
    });
}

#[test]
fn test_if_ready() {
    let mut block = MaybeBlock {
        value: 18,
        attempts_per_item: 2,
        remaining_attemts: 2,
    };

    // First poll attempt should not invoke the closure
    block
        .poll_me()
        .if_ready(|_| {
            panic!("I am not ready yet");
        })
        .unwrap();
    // But second one should
    block
        .poll_me()
        .if_ready(|value| {
            assert_eq!(value, 18);
            Ok(())
        })
        .unwrap()
}
