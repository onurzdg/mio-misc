//! Thread safe queues that that trigger notifications on `mio::Waker`
use crate::NotificationId;
use crossbeam_queue::{ArrayQueue, SegQueue};
use mio::Waker;
use std::sync::Arc;
use std::{error, fmt, io};

///
pub type Result = std::result::Result<(), NotificationError<NotificationId>>;

/// Represents the side that notifies
pub trait Notifier: Send + Sync {
    /// Notifies `Poll`
    fn notify(&self, id: NotificationId) -> Result;
}

/// Represents the side that receives event notifications
pub trait NotificationReceiver: Send + Sync {
    /// Retrieves the next notification, if there's any
    fn receive(&self) -> Option<NotificationId>;
    /// Returns number of notifications
    fn len(&self) -> usize;
    /// Returns `true` if the queue is empty.
    fn is_empty(&self) -> bool;
}

/// An unbounded queue that helps with simulation of registering event sources with `Poll`.
/// It keeps track of `NotificationId`s associated with `Waker`
pub struct NotificationQueue {
    /// Waker to notify Poll
    waker: Arc<Waker>,
    /// Queue of `NotificationId`s
    queue: SegQueue<NotificationId>,
}

impl NotificationQueue {
    /// Creates a notification queue
    pub fn new(waker: Arc<Waker>) -> NotificationQueue {
        NotificationQueue {
            waker,
            queue: SegQueue::new(),
        }
    }

    /// Queues the `NotificationId` and notifies the `Poll` associated with `Waker`
    pub fn push(&self, id: NotificationId) -> io::Result<()> {
        self.queue.push(id);
        self.waker.wake()
    }

    /// Attempts to remove an element from the queue
    /// If the queue is empty, None is returned.
    pub fn pop(&self) -> Option<NotificationId> {
        self.queue.pop()
    }

    /// Returns `true` if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns queue length
    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

impl Notifier for NotificationQueue {
    fn notify(&self, id: NotificationId) -> Result {
        self.push(id).map_err(From::from)
    }
}

impl NotificationReceiver for NotificationQueue {
    fn receive(&self) -> Option<NotificationId> {
        self.pop()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

/// A bounded queue that helps with simulation of registering event sources with `Poll`.
/// It keeps track of `NotificationId`s associated with Waker
pub struct BoundedNotificationQueue {
    /// Waker to notify Poll
    waker: Arc<Waker>,
    /// Queue of `NotificationId`'s
    queue: ArrayQueue<NotificationId>,
}

impl BoundedNotificationQueue {
    /// Creates the queue
    pub fn new(size: usize, waker: Arc<Waker>) -> BoundedNotificationQueue {
        BoundedNotificationQueue {
            waker,
            queue: ArrayQueue::new(size),
        }
    }

    /// Queues the `NotificationId` and notifies the `Poll` associated with `Waker`
    pub fn push(&self, id: NotificationId) -> Result {
        self.queue
            .push(id)
            .map_err(|id| NotificationError::Full(id))
            .and_then(|_| self.waker.wake().map_err(From::from))
    }

    /// Attempts to remove an element from the queue
    /// If the queue is empty, None is returned.
    pub fn pop(&self) -> Option<NotificationId> {
        self.queue.pop()
    }

    /// Returns `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns queue length
    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

impl Notifier for BoundedNotificationQueue {
    fn notify(&self, id: NotificationId) -> Result {
        self.push(id)
    }
}

impl NotificationReceiver for BoundedNotificationQueue {
    fn receive(&self) -> Option<NotificationId> {
        self.pop()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// An error returned from the `SyncEventNotificationQueue::push` function.
pub enum NotificationError<T> {
    /// An IO error.
    Io(io::Error),

    /// Notification could not be sent because the queue is full.
    Full(T),
}

/*
 *
 * ===== Implement Error conversions =====
 *
 */

impl<T> From<io::Error> for NotificationError<T> {
    fn from(src: io::Error) -> Self {
        NotificationError::Io(src)
    }
}

/*
 *
 * ===== Implement Error, Debug, and Display for Errors =====
 *
 */

impl<T> error::Error for NotificationError<T> {}

impl<T> fmt::Debug for NotificationError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_sync_notification_error(self, f)
    }
}

impl<T> fmt::Display for NotificationError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_sync_notification_error(self, f)
    }
}

#[inline]
fn format_sync_notification_error<T>(
    e: &NotificationError<T>,
    f: &mut fmt::Formatter,
) -> fmt::Result {
    match e {
        NotificationError::Io(ref io_err) => write!(f, "{}", io_err),
        NotificationError::Full(..) => write!(f, "Full"),
    }
}
