//! Thread safe communication channel
use std::error;
use std::sync::{mpsc, Arc};
use std::{fmt, io};
use crate::NotificationId;
use crate::queue::{NotificationQueue, SyncNotificationQueue, SyncNotificationError};

/// Creates a new asynchronous channel, where the `Sender::send` function, in addition to sending a message,
/// queues `NotificationId` and thereby trigger notification on `Poll`
pub fn channel<T>(queue: Arc<NotificationQueue>, id: NotificationId) -> (Sender<T>, mpsc::Receiver<T>) {
    let (tx, rx) = mpsc::channel();
    let tx = Sender { queue: Arc::clone(&queue), tx, id};
    (tx, rx)
}

/// Creates a new synchronous channel, where the `SyncSender::send` function, in addition to sending a message,
/// queues `NotificationId` and thereby trigger notification on `Poll`
pub fn sync_channel<T>(queue: Arc<SyncNotificationQueue>, id: NotificationId, bound_size: usize) -> (SyncSender<T>, mpsc::Receiver<T>) {
    let (tx, rx) = mpsc::sync_channel(bound_size);
    let tx = SyncSender { queue: Arc::clone(&queue), tx, id};
    (tx, rx)
}

/// The sending half of a channel.
pub struct Sender<T> {
    tx: mpsc::Sender<T>,
    queue: Arc<NotificationQueue>,
    id: NotificationId
}

impl<T> Sender<T> {
    /// Attempts to send a value on this channel, returning it back if it could not be sent.
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        self.tx.send(t).map_err(SendError::from)?;
        self.queue.push(self.id).map_err(SendError::from)
    }
}

/// The sending half of a synchronous channel.
pub struct SyncSender<T> {
    tx: mpsc::SyncSender<T>,
    queue: Arc<SyncNotificationQueue>,
    id: NotificationId
}

impl<T> SyncSender<T> {
    /// Sends a value on this synchronous channel.
    ///
    /// This function will *block* until space in the internal buffer becomes
    /// available or a receiver is available to hand off the message to.
    ///
    pub fn send(&self, t: T) -> Result<(), SyncSendError> {
        self.tx.send(t).map_err(From::from)
            .and(self.queue.push(self.id).map_err(From::from))
    }

    /// Attempts to send a value on this channel without blocking.
    ///
    /// This method differs from `send` by returning immediately if the channel's
    /// buffer is full or no receiver is waiting to acquire some data.
    pub fn try_send(&self, t: T) -> Result<(), SyncTrySendError> {
        self.tx.try_send(t).map_err(From::from).and(
            self.queue.push(self.id).map_err(From::from)
        )
    }
}

/// An error returned from the `Sender::send`
pub enum SendError<T> {
    /// An IO error.
    Io(io::Error),

    /// The receiving half of the channel has disconnected.
    Disconnected(T)
}

/// An error returned from the `SyncSender::send` function.
pub enum SyncSendError {
    /// An IO error.
    Io(io::Error),

    /// The receiving half of the channel has disconnected.
    Disconnected,

    /// Failed to add notification
    NotificationFailed
}

/// An error returned from the `SyncSender::try_send` function
pub enum SyncTrySendError {
    /// An IO error.
    Io(io::Error),

    /// The receiving half of the channel has disconnected.
    Disconnected,

    /// Failed to add notification
    NotificationFailed,

    /// Full
    Full
}

/// An error returned from the `SyncSender::try_send` function.
pub enum TrySendError<T> {
    /// An IO error.
    Io(io::Error),

    /// Data could not be sent because it would require the callee to block.
    Full(T),

    /// The receiving half of the channel has disconnected.
    Disconnected(T),
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        Sender {
            tx: self.tx.clone(),
            queue: Arc::clone(&self.queue),
            id: self.id
        }
    }
}

impl<T> Clone for SyncSender<T> {
    fn clone(&self) -> SyncSender<T> {
        SyncSender {
            tx: self.tx.clone(),
            queue: Arc::clone(&self.queue),
            id: self.id
        }
    }
}

/*
 *
 * ===== Error conversions =====
 *
 */

impl<T> From<mpsc::SendError<T>> for SendError<T> {
    fn from(src: mpsc::SendError<T>) -> Self {
        SendError::Disconnected(src.0)
    }
}

impl<T> From<io::Error> for SendError<T> {
    fn from(src: io::Error) -> Self {
        SendError::Io(src)
    }
}

impl<T> From<mpsc::SendError<T>> for SyncSendError {
    fn from(_: mpsc::SendError<T>) -> Self {
        SyncSendError::Disconnected
    }
}

impl From<io::Error> for SyncSendError {
    fn from(src: io::Error) -> Self {
        SyncSendError::Io(src)
    }
}

impl From<SyncNotificationError<NotificationId>> for SyncSendError {
    fn from(_: SyncNotificationError<NotificationId>) -> Self {
        SyncSendError::NotificationFailed
    }
}

impl<T> From<mpsc::TrySendError<T>> for TrySendError<T> {
    fn from(src: mpsc::TrySendError<T>) -> Self {
        match src {
            mpsc::TrySendError::Full(v) => TrySendError::Full(v),
            mpsc::TrySendError::Disconnected(v) => TrySendError::Disconnected(v),
        }
    }
}

impl<T> From<SyncNotificationError<T>> for SyncTrySendError {
    fn from(src: SyncNotificationError<T>) -> Self {
        match src {
            SyncNotificationError::Full(_) => SyncTrySendError::NotificationFailed,
            SyncNotificationError::Io(src) => SyncTrySendError::Io(src),
        }
    }
}

impl <T> From<mpsc::TrySendError<T>> for SyncTrySendError {
    fn from(src: mpsc::TrySendError<T>) -> Self {
        match src {
            mpsc::TrySendError::Full(_) => SyncTrySendError::Full,
            mpsc::TrySendError::Disconnected(_) => SyncTrySendError::Disconnected,
        }
    }
}

impl From<io::Error> for SyncTrySendError {
    fn from(src: io::Error) -> Self {
        SyncTrySendError::Io(src)
    }
}

impl<T> From<mpsc::SendError<T>> for TrySendError<T> {
    fn from(src: mpsc::SendError<T>) -> Self {
        TrySendError::Disconnected(src.0)
    }
}

impl<T> From<io::Error> for TrySendError<T> {
    fn from(src: io::Error) -> Self {
        TrySendError::Io(src)
    }
}

/*
 *
 * ===== Implement Error, Debug and Display for Errors =====
 *
 */

impl<T> error::Error for SendError<T> {}

impl error::Error for SyncSendError {}

impl<T> error::Error for TrySendError<T> {}

impl error::Error for SyncTrySendError {}


impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_send_error(self, f)
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_send_error(self, f)
    }
}

impl fmt::Debug for SyncSendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_sync_send_error(self, f)
    }
}

impl fmt::Display for SyncSendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_sync_send_error(self, f)
    }
}

impl fmt::Debug for SyncTrySendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_sync_try_send_error(self, f)
    }
}

impl fmt::Display for SyncTrySendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_sync_try_send_error(self, f)
    }
}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_try_send_error(self, f)
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        format_try_send_error(self, f)
    }
}

#[inline]
fn format_send_error<T>(e: &SendError<T>, f: &mut fmt::Formatter) -> fmt::Result {
    match e {
        SendError::Io(ref io_err) => write!(f, "{}", io_err),
        SendError::Disconnected(..) => write!(f, "Disconnected"),
    }
}

#[inline]
fn format_sync_send_error(e: &SyncSendError, f: &mut fmt::Formatter) -> fmt::Result {
    match e {
        SyncSendError::Io(ref io_err) => write!(f, "{}", io_err),
        SyncSendError::Disconnected => write!(f, "Disconnected"),
        SyncSendError::NotificationFailed => write!(f, "Notification failed"),
    }
}

#[inline]
fn format_sync_try_send_error(e: &SyncTrySendError, f: &mut fmt::Formatter) -> fmt::Result {
    match e {
        SyncTrySendError::Io(ref io_err) => write!(f, "{}", io_err),
        SyncTrySendError::Disconnected => write!(f, "Disconnected"),
        SyncTrySendError::NotificationFailed => write!(f, "Notification failed"),
        SyncTrySendError::Full => write!(f, "Channel full"),
    }
}

#[inline]
fn format_try_send_error<T>(e: &TrySendError<T>, f: &mut fmt::Formatter) -> fmt::Result {
    match e {
        TrySendError::Io(ref io_err) => write!(f, "{}", io_err),
        TrySendError::Full(..) => write!(f, "Channel full"),
        TrySendError::Disconnected(..) => write!(f, "Disconnected"),
    }
}
