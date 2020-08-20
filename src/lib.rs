//! Extra components for use with Mio.
#![deny(missing_docs)]
extern crate mio;
extern crate crossbeam;

use std::sync::atomic::{AtomicUsize, Ordering};

pub mod channel;
pub mod scheduler;
pub mod poll;
pub mod queue;

static NOTIFICATION_ID: AtomicUsize = AtomicUsize::new(1);

/// NotificationId
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NotificationId {id: usize}

impl NotificationId {
    /// Generates next `NotificationId`, which is guaranteed to be unique
    pub fn gen_next() -> NotificationId {
        let id = NOTIFICATION_ID.fetch_add(1, Ordering::SeqCst);
        NotificationId{id}
    }

    /// Returns id
    pub fn id(&self) -> usize {
        self.id
    }
}
