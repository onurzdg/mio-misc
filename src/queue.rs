//! Thread safe queue that helps with registration of event sources with `Poll`
use crossbeam_queue::{ArrayQueue, SegQueue};
use crate::NotificationId;
use std::sync::Arc;
use std::{io, fmt, error};
use mio::Waker;

/// An unbounded queue
/// Helps with simulation of registering event sources with `Poll`
/// Keeps track of `NotificationId`s associated with `Waker`
pub struct NotificationQueue {
    /// Waker to notify Poll
    waker: Arc<Waker>,
    /// Queue of `NotificationId`s
    queue: SegQueue<NotificationId>,
}

impl NotificationQueue {
    /// Creates the queue
    pub fn new (waker: Arc<Waker>) -> NotificationQueue {
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

    /// Attempts to remove an element from the queue.
    /// If the queue is empty, None is returned.
    pub fn pop(&self) -> Option<NotificationId> {
        self.queue.pop().ok()
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

/// An bounded queue
/// Helps with simulation of registering event sources with `Poll`
/// Keeps track of `NotificationId`s associated with Waker
pub struct SyncNotificationQueue {
    /// Waker to notify Poll
    waker: Arc<Waker>,
    /// Queue of `NotificationId`'s
    queue: ArrayQueue<NotificationId>
}

impl SyncNotificationQueue {
    /// Creates the queue
    pub fn new(size: usize, waker: Arc<Waker>) -> SyncNotificationQueue {
        SyncNotificationQueue {
            waker,
            queue: ArrayQueue::new(size)
        }
    }

    /// Queues the `NotificationId` and notifies the `Poll` associated with `Waker`
    pub fn push(&self, id: NotificationId) -> Result<(), SyncNotificationError<NotificationId>> {
        self.queue.push(id).map_err(From::from).and(
            self.waker.wake().map_err(From::from)
        )
    }

    /// Attempts to remove an element from the queue.
    /// If the queue is empty, None is returned.
    pub fn pop(&self) -> Option<NotificationId> {
        self.queue.pop().ok()
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

/// An error returned from the `SyncEventNotificationQueue::push` function.
pub enum SyncNotificationError<T> {
    /// An IO error.
    Io(io::Error),

    /// Notification could not be sent because the queue is full.
    Full(T),
}

impl<T> From<crossbeam_queue::PushError<T>> for SyncNotificationError<T> {
    fn from(src: crossbeam_queue::PushError<T>) -> Self {
        SyncNotificationError::Full(src.0)
    }
}

impl<T> error::Error for SyncNotificationError<T> {}

impl<T> fmt::Debug for SyncNotificationError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_sync_notification_error(self, f)
    }
}

impl<T> fmt::Display for SyncNotificationError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_sync_notification_error(self, f)
    }
}

impl<T> From<io::Error> for SyncNotificationError<T> {
    fn from(src: io::Error) -> Self {
        SyncNotificationError::Io(src)
    }
}

#[inline]
fn format_sync_notification_error<T>(e: &SyncNotificationError<T>, f: &mut fmt::Formatter) -> fmt::Result {
    match e {
        SyncNotificationError::Io(ref io_err) => write!(f, "{}", io_err),
        SyncNotificationError::Full(..) => write!(f, "Full")
    }
}