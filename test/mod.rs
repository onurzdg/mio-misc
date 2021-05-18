extern crate mio;
extern crate mio_misc;

use mio::{Interest, Token};
use mio_misc::poll::Poll;
use std::time::Duration;

mod test_poll_channel;
mod test_scheduler;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TestEvent {
    token: Token,
    interest: Interest,
}

pub fn expect_events(poll: &mut Poll, poll_try_count: usize, mut expected: Vec<TestEvent>) {
    let poll_timeout = Duration::from_secs(2);

    for _ in 0..poll_try_count {
        let events = poll.wait(poll_timeout).unwrap();
        for event in events.iter() {
            let pos_opt = expected
                .iter()
                .position(|exp_event| event.token() == exp_event.token);
            if let Some(pos) = pos_opt {
                expected.remove(pos);
            }
        }

        if expected.is_empty() {
            break;
        }
    }

    assert!(
        expected.is_empty(),
        "The following expected events were not found: {:?}",
        expected
    );
}
