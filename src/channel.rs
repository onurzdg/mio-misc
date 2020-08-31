//! Thread safe communication channels
use crate::queue::{NotificationError, Notifier};
use crate::NotificationId;
use crossbeam::channel as beamchannel;
use std::error;
use std::sync::{mpsc, Arc};
use std::{fmt, io};

/// Creates a new asynchronous/unbounded channel, where the `Sender::send` function, in addition to sending a message,
/// triggers a notification on `Poll`
pub fn channel<T>(
    notifier: Arc<dyn Notifier>,
    id: NotificationId,
) -> (Sender<T>, mpsc::Receiver<T>) {
    let (tx, rx) = mpsc::channel();
    let tx = Sender { notifier, tx, id };
    (tx, rx)
}

/// Creates a new synchronous channel, where the `SyncSender::send` function, in addition to sending a message,
/// triggers a notification on `Poll`
pub fn sync_channel<T>(
    notifier: Arc<dyn Notifier>,
    id: NotificationId,
    bound_size: usize,
) -> (SyncSender<T>, mpsc::Receiver<T>) {
    let (tx, rx) = mpsc::sync_channel(bound_size);
    let tx = SyncSender { notifier, tx, id };
    (tx, rx)
}

/// Creates a new asynchronous/unbounded crossbeam channel, where the `Sender::send` function, in addition to sending a message,
/// triggers a notification on `Poll`
pub fn crossbeam_channel_unbounded<T>(
    notifier: Arc<dyn Notifier>,
    id: NotificationId,
) -> (CrossbeamSender<T>, beamchannel::Receiver<T>) {
    let (tx, rx) = beamchannel::unbounded();
    let tx = CrossbeamSender { notifier, tx, id };
    (tx, rx)
}

/// Creates a new synchronous/bounded crossbeam channel, where the `Sender::send` function, in addition to sending a message,
/// triggers a notification on `Poll`
pub fn crossbeam_channel_bounded<T>(
    notifier: Arc<dyn Notifier>,
    id: NotificationId,
    size: usize,
) -> (CrossbeamSender<T>, beamchannel::Receiver<T>) {
    let (tx, rx) = beamchannel::bounded(size);
    let tx = CrossbeamSender { notifier, tx, id };
    (tx, rx)
}

/// The sending half of a channel.
pub struct Sender<T> {
    tx: mpsc::Sender<T>,
    notifier: Arc<dyn Notifier>,
    id: NotificationId,
}

impl<T> Sender<T> {
    /// Attempts to send a value on this channel, returning it back if it could not be sent.
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        self.tx.send(t).map_err(SendError::from)?;
        self.notifier.notify(self.id).map_err(SendError::from)
    }
}

/// The sending half of a channel crossbeam channel
pub struct CrossbeamSender<T> {
    tx: beamchannel::Sender<T>,
    notifier: Arc<dyn Notifier>,
    id: NotificationId,
}

impl<T> CrossbeamSender<T> {
    /// Attempts to send a value on this channel, returning it back if it could not be sent.
    /// For bounded channels, it will block.
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        self.tx.send(t).map_err(SendError::from)?;
        self.notifier.notify(self.id).map_err(SendError::from)
    }

    /// Attempts to send a value on this channel without blocking.
    ///
    /// This method differs from `send` by returning immediately if the channel's
    /// buffer is full or no receiver is waiting to acquire some data.
    pub fn try_send(&self, t: T) -> Result<(), TrySendError<T>> {
        self.tx
            .try_send(t)
            .map_err(From::from)
            .and_then(|_| self.notifier.notify(self.id).map_err(From::from))
    }
}

/// The sending half of a synchronous channel.
pub struct SyncSender<T> {
    tx: mpsc::SyncSender<T>,
    notifier: Arc<dyn Notifier>,
    id: NotificationId,
}

impl<T> SyncSender<T> {
    /// Sends a value on this synchronous channel.
    ///
    /// This function will *block* until space in the internal buffer becomes
    /// available or a receiver is available to hand off the message to.
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        self.tx
            .send(t)
            .map_err(From::from)
            .and_then(|_| self.notifier.notify(self.id).map_err(From::from))
    }

    /// Attempts to send a value on this channel without blocking.
    ///
    /// This method differs from `send` by returning immediately if the channel's
    /// buffer is full or no receiver is waiting to acquire some data.
    pub fn try_send(&self, t: T) -> Result<(), TrySendError<T>> {
        self.tx
            .try_send(t)
            .map_err(From::from)
            .and_then(|_| self.notifier.notify(self.id).map_err(From::from))
    }
}

/// An error returned from the `Sender::send`
pub enum SendError<T> {
    /// An IO error.
    Io(io::Error),

    /// The receiving half of the channel has disconnected.
    Disconnected(T),

    /// Underlying notification queue is full
    NotificationQueueFull,
}

/// An error returned from the `SyncSender::try_send` function.
pub enum TrySendError<T> {
    /// An IO error.
    Io(io::Error),

    /// Data could not be sent over the channel because it would require the callee to block.
    Full(T),

    /// The receiving half of the channel has disconnected.
    Disconnected(T),

    /// Underlying notification queue is full
    NotificationQueueFull,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        Sender {
            tx: self.tx.clone(),
            notifier: Arc::clone(&self.notifier),
            id: self.id,
        }
    }
}

impl<T> Clone for SyncSender<T> {
    fn clone(&self) -> SyncSender<T> {
        SyncSender {
            tx: self.tx.clone(),
            notifier: Arc::clone(&self.notifier),
            id: self.id,
        }
    }
}

/*
 *
 * ===== Implement Error conversions =====
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

impl<T> From<beamchannel::SendError<T>> for SendError<T> {
    fn from(src: beamchannel::SendError<T>) -> Self {
        SendError::Disconnected(src.0)
    }
}

impl<T> From<NotificationError<NotificationId>> for SendError<T> {
    fn from(_: NotificationError<NotificationId>) -> Self {
        SendError::NotificationQueueFull
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

impl<T> From<NotificationError<NotificationId>> for TrySendError<T> {
    fn from(_: NotificationError<NotificationId>) -> Self {
        TrySendError::NotificationQueueFull
    }
}

impl<T> From<beamchannel::TrySendError<T>> for TrySendError<T> {
    fn from(src: beamchannel::TrySendError<T>) -> Self {
        match src {
            beamchannel::TrySendError::Full(v) => TrySendError::Full(v),
            beamchannel::TrySendError::Disconnected(v) => TrySendError::Disconnected(v),
        }
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
 * ===== Implement Error, Debug, and Display for Errors =====
 *
 */

impl<T> error::Error for SendError<T> {}

impl<T> error::Error for TrySendError<T> {}

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
        SendError::Disconnected(_) => write!(f, "Disconnected"),
        SendError::NotificationQueueFull => write!(f, "Notification queue full"),
    }
}

#[inline]
fn format_try_send_error<T>(e: &TrySendError<T>, f: &mut fmt::Formatter) -> fmt::Result {
    match e {
        TrySendError::Io(ref io_err) => write!(f, "{}", io_err),
        TrySendError::Full(..) => write!(f, "Channel full"),
        TrySendError::Disconnected(..) => write!(f, "Disconnected"),
        TrySendError::NotificationQueueFull => write!(f, "Notification queue full"),
    }
}
