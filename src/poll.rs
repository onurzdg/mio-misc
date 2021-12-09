//! Wrapper around `mio::Poll` to offer a slightly more convenient API
use mio::Registry;
use std::io;
use std::time::Duration;

/// Encapsulates `mio::Poll` and `mio::Events`
#[derive(Debug)]
pub struct Poll {
    poll: mio::Poll,
    events: mio::Events,
}

impl Poll {
    /// Creates a `Poll`
    pub fn with_capacity(capacity: usize) -> io::Result<Poll> {
        let poll = mio::Poll::new()?;
        Ok(Poll {
            poll,
            events: mio::Events::with_capacity(capacity),
        })
    }

    /// Polls until the optionally provided duration and returns `Events`
    pub fn poll<I>(&mut self, timeout: I) -> io::Result<&mio::Events>
    where
        I: Into<Option<Duration>>,
    {
        let _ = self.poll.poll(&mut self.events, timeout.into())?;
        Ok(&self.events)
    }

    /// Clears polled events
    pub fn clear(&mut self) {
        self.events.clear()
    }

    /// Gives access to polled events
    pub fn polled_events(&self) -> &mio::Events {
        &self.events
    }
    /// Returns registry
    pub fn registry(&self) -> &Registry {
        self.poll.registry()
    }
}
