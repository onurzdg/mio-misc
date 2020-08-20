//! Thread safe scheduler
use std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicBool};
use std::{
    thread,
    time::{Duration, Instant},
};
use crate::NotificationId;
use crate::queue::{NotificationQueue, SyncNotificationQueue};
use std::ops::Sub;

/// Notification scheduler that periodically adds notification to the queue
pub struct NotificationScheduler {
    handle: SchedulerHandle
}

impl NotificationScheduler {
    /// Adds notification to the queue at fixed intervals with an optional initial delay
    pub fn notify_with_fixed_interval<I: Into<Option<Duration>>>(id: NotificationId, queue: Arc<NotificationQueue>, interval: Duration, initial_delay: I) -> NotificationScheduler
        where I: Into<Option<Duration>> {
        NotificationScheduler {
            handle: Scheduler::schedule_with_fixed_interval(interval,
                                                            initial_delay,
                                                            move || { let _ = queue.push(id); },
            )
        }
    }

    /// Adds a one-time notification to the queue after an initial delay
    pub fn notify_once_after_delay(id: NotificationId, queue: Arc<NotificationQueue>, delay: Duration) -> NotificationScheduler {
        NotificationScheduler {
            handle: Scheduler::schedule_one_time(delay,
                                                 move || { let _ = queue.push(id); },
            )
        }
    }

    /// Adds notification to the queue at fixed intervals with an optional initial delay
    pub fn notify_with_fixed_interval_with_sync_queue<I>(id: NotificationId, queue: Arc<SyncNotificationQueue>, interval: Duration, initial_delay: I) -> NotificationScheduler
        where I: Into<Option<Duration>> {
        NotificationScheduler {
            handle: Scheduler::schedule_with_fixed_interval(interval,
                                                            initial_delay,
                                                            move || { let _ = queue.push(id); },
            )
        }
    }

    /// Adds a one-time notification to the queue after an initial delay
    pub fn notify_once_after_delay_with_sync_queue(id: NotificationId, queue: Arc<SyncNotificationQueue>, delay: Duration) -> NotificationScheduler {
        NotificationScheduler {
            handle: Scheduler::schedule_one_time(delay,
                                                 move || { let _ = queue.push(id); },
            )
        }
    }

    /// Cancels scheduled runs of the passed function unless it was already cancelled,
    /// in which case it returns false.
    pub fn cancel(self) -> thread::Result<()> {
        self.handle.cancel()
    }
}

/// Handle to the created schedule
pub struct SchedulerHandle {
    cancel: Arc<AtomicBool>,
    thread_handle: thread::JoinHandle<()>,
}

impl SchedulerHandle {
    /// Cancels scheduled runs of the passed function unless it was already cancelled,
    /// in which case it returns false.
    pub fn cancel(self) -> thread::Result<()> {
        self.cancel.store(true, Ordering::SeqCst);
        self.thread_handle.thread().unpark();
        self.thread_handle.join()
    }
}

/// Runs the provided function with fixed intervals
pub struct Scheduler {}

impl Scheduler {
    /// Schedules function runs with fixed delay
    pub fn schedule_with_fixed_interval<F, I>(interval: Duration, initial_delay: I, f: F) -> SchedulerHandle
        where F: Fn() + Send + 'static,
              I: Into<Option<Duration>> {
        let initial_delay = initial_delay.into();
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = Arc::clone(&cancel);
        let thread_handle = thread::spawn(move || {
            if let Some(delay) = initial_delay {
                let instant = Instant::now();
                while !thread_cancel.load(Ordering::SeqCst) {
                    thread::park_timeout(delay);
                    let elapsed = instant.elapsed();
                    if elapsed.ge(&delay) {
                        break;
                    }
                }
            }

            let mut instant = Instant::now();
            let mut interval = interval;
            while !thread_cancel.load(Ordering::SeqCst) {
                thread::park_timeout(interval);
                if !thread_cancel.load(Ordering::SeqCst) { // still not cancelled?
                    // protects against spurious wake-ups
                    let elapsed = instant.elapsed();
                    if elapsed.ge(&interval) { // interval exceeded?
                        f();
                        instant = Instant::now();
                    } else {
                        interval = interval.sub(elapsed);
                    }
                }
            }
        });
        SchedulerHandle {
            cancel,
            thread_handle,
        }
    }

    /// Schedules one-off function run
    pub fn schedule_one_time<F: Fn() + Send + 'static>(delay: Duration, f: F) -> SchedulerHandle {
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = Arc::clone(&cancel);
        let thread_handle = thread::spawn(move || {
            let instant = Instant::now();
            let mut delay = delay;
            while !thread_cancel.load(Ordering::SeqCst) {
                thread::park_timeout(delay);
                if !thread_cancel.load(Ordering::SeqCst) { // still not cancelled?
                    // protects against spurious wake-ups
                    let elapsed = instant.elapsed();
                    if elapsed.ge(&delay) { // interval exceeded?
                        f();
                        break;
                    } else {
                        delay = delay.sub(elapsed);
                    }
                }
            }
        });
        SchedulerHandle {
            cancel,
            thread_handle,
        }
    }
}