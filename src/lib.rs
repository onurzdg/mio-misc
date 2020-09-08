//! Miscellaneous components for use with Mio.
#![deny(missing_docs)]
extern crate crossbeam;
extern crate mio;

#[macro_use]
extern crate log;

use std::sync::atomic::{AtomicU32, Ordering};

pub mod channel;
pub mod poll;
pub mod queue;
pub mod scheduler;

static NEXT_NOTIFICATION_ID: AtomicU32 = AtomicU32::new(1);

/// Used while sending notifications
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NotificationId(u32);

impl NotificationId {
    /// Generates the next `NotificationId`, which is guaranteed to be unique
    pub fn gen_next() -> NotificationId {
        let id = NEXT_NOTIFICATION_ID.fetch_add(1, Ordering::SeqCst);
        NotificationId(id)
    }

    /// Returns id
    pub fn id(&self) -> u32 {
        self.0
    }
}
