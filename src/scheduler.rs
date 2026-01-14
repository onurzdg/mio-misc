//! Thread safe scheduler
use crate::queue::Notifier;
use crate::NotificationId;
use crossbeam_queue::SegQueue;
use std::cmp::Ordering as CmpOrdering;
use std::collections::{BTreeSet, HashSet};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Sub};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::{
    fmt, thread,
    time::{Duration, Instant},
};

/// Schedules notification deliveries
#[derive(Debug)]
pub struct NotificationScheduler {
    notifier: Arc<dyn Notifier>,
    scheduler: Arc<Scheduler>,
}

impl NotificationScheduler {
    /// Creates a new scheduler that uses the provided notifier to deliver notifications
    pub fn new(notifier: Arc<dyn Notifier>, scheduler: Arc<Scheduler>) -> NotificationScheduler {
        NotificationScheduler {
            notifier,
            scheduler,
        }
    }

    /// Schedules recurring notification deliveries with fixed intervals
    pub fn notify_with_fixed_interval<I: Into<Option<Duration>>>(
        &self,
        id: NotificationId,
        interval: Duration,
        initial_delay: I,
        name: Option<String>,
    ) -> ScheduleEntryId {
        let notifier = Arc::clone(&self.notifier);
        let entry = ScheduleEntry::with_interval(interval, initial_delay, name, move || {
            let _ = notifier.notify(id);
        });
        let id = entry.id;
        self.scheduler.schedule(entry);
        id
    }

    /// Schedules a one-time notification delivery
    pub fn notify_once_after_delay(
        &self,
        id: NotificationId,
        delay: Duration,
        name: Option<String>,
    ) -> ScheduleEntryId {
        let notifier = Arc::clone(&self.notifier);
        let entry = ScheduleEntry::one_time(delay, name, move || {
            let _ = notifier.notify(id);
        });
        let id = entry.id;
        self.scheduler.schedule(entry);
        id
    }

    /// Cancels future notification(s)
    pub fn cancel(&self, id: ScheduleEntryId) {
        self.scheduler.cancel(id);
    }
}

type Callback = dyn Fn() + Send + Sync + 'static;

/// Entry associated with callback
#[derive(Clone)]
pub struct ScheduleEntry {
    start: Instant,
    /// The interval with which to run the callback. No interval means only one-time run
    interval: Option<Duration>,
    callback: Arc<Callback>,
    /// The assigned name of the entry for debugging purposes
    pub name: Option<String>,
    /// Entry Id
    pub id: ScheduleEntryId,
}

impl ScheduleEntry {
    /// Creates an entry to run the callback repeatedly with a fixed delay
    pub fn with_interval<I, F>(
        interval: Duration,
        initial_delay: I,
        name: Option<String>,
        callback: F,
    ) -> ScheduleEntry
    where
        I: Into<Option<Duration>>,
        F: Fn() + Send + Sync + 'static,
    {
        let now = Instant::now();
        ScheduleEntry {
            start: initial_delay.into().map(|d| now.add(d)).unwrap_or(now),
            interval: Some(interval),
            callback: Arc::new(callback),
            name,
            id: ScheduleEntryId::gen_next(),
        }
    }

    /// Creates an entry to run the callback only once after a given delay
    pub fn one_time<F>(delay: Duration, name: Option<String>, callback: F) -> ScheduleEntry
    where
        F: Fn() + Send + Sync + 'static,
    {
        ScheduleEntry {
            start: Instant::now().add(delay),
            interval: None,
            callback: Arc::new(callback),
            name,
            id: ScheduleEntryId::gen_next(),
        }
    }
}

impl fmt::Debug for ScheduleEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScheduleEntry")
            .field("start", &self.start)
            .field("interval", &self.interval)
            .field("name", &self.name)
            .field("id", &self.id)
            .finish()
    }
}

impl Hash for ScheduleEntry {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.start.hash(hasher);
        self.id.hash(hasher);
    }
}

impl Eq for ScheduleEntry {}

impl PartialEq for ScheduleEntry {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.id == other.id
    }
}

impl Ord for ScheduleEntry {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        self.start.cmp(&other.start).then(self.id.cmp(&other.id))
    }
}

impl PartialOrd for ScheduleEntry {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

static NEXT_SCHEDULE_ENTRY_ID: AtomicU32 = AtomicU32::new(1);

/// Id associated with an entry
#[derive(Copy, Clone, Debug, PartialEq, Ord, PartialOrd, Eq, Hash)]
pub struct ScheduleEntryId(u32);

impl ScheduleEntryId {
    /// Generates next `ScheduleEntryId`, which is guaranteed to be unique
    pub fn gen_next() -> ScheduleEntryId {
        let id = NEXT_SCHEDULE_ENTRY_ID.fetch_add(1, Ordering::SeqCst);
        ScheduleEntryId(id)
    }

    /// Returns id
    pub fn id(&self) -> u32 {
        self.0
    }
}

/// Scheduler Status
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SchedulerStatus {
    /// Currently executing an entry
    Active,
    /// Waiting for new entries to be scheduled
    Parked,
    /// Waiting to execute entries in the queue at the scheduled intervals
    ParkedTimeout,
}

/// Single-threaded scheduler that prioritizes "cancels" over schedule executions, hence multiple queues
#[derive(Debug)]
pub struct Scheduler {
    shutdown: Arc<AtomicBool>,
    thread_handle: JoinHandle<()>,
    schedule_queue: Arc<SegQueue<ScheduleEntry>>,
    cancel_queue: Arc<SegQueue<ScheduleEntryId>>,
    name: String,
    status: Arc<RwLock<SchedulerStatus>>,
    entry_count: Arc<AtomicU32>,
}

impl Default for Scheduler {
    fn default() -> Scheduler {
        Scheduler::new(None)
    }
}

// Helps distinguish different scheduler creations
static SCHEDULER_THREAD_ID: AtomicU32 = AtomicU32::new(1);

impl Scheduler {
    /// Returns the name of the scheduler
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Schedules entry for execution(s)
    pub fn schedule(&self, entry: ScheduleEntry) {
        self.schedule_queue.push(entry);
        self.thread_handle.thread().unpark();
    }

    /// Cancels future execution(s)
    pub fn cancel(&self, id: ScheduleEntryId) {
        self.cancel_queue.push(id);
        self.thread_handle.thread().unpark();
    }

    /// Returns the scheduler's current status
    pub fn status(&self) -> SchedulerStatus {
        *(self.status.read().unwrap())
    }

    /// Number of current entries
    pub fn entry_count(&self) -> u32 {
        self.entry_count.load(Ordering::SeqCst)
    }

    /// Creates a scheduler
    pub fn new(name: Option<String>) -> Scheduler {
        let t_id = SCHEDULER_THREAD_ID.fetch_add(1, Ordering::SeqCst);
        let name_prefix = "mio-misc-scheduler";
        let name = name
            .map(|n| format!("{}-{}-{}", name_prefix, n, t_id))
            .unwrap_or_else(|| format!("{}-{}", name_prefix, t_id));
        let name_clone = name.clone();

        let shut_down = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shut_down);
        let entry_count = Arc::new(AtomicU32::new(0));
        let entry_count_clone = Arc::clone(&entry_count);
        let schedule_queue = Arc::new(SegQueue::new());
        let schedule_queue_clone = Arc::clone(&schedule_queue);
        let cancel_queue = Arc::new(SegQueue::new());
        let cancel_queue_clone = Arc::clone(&cancel_queue);
        let status = Arc::new(RwLock::new(SchedulerStatus::Active));
        let status_clone = Arc::clone(&status);
        let thread_handle = thread::Builder::new()
            .name(name.clone())
            .spawn(move || {
                let mut entries: BTreeSet<ScheduleEntry> = BTreeSet::new();
                let mut entries_to_cancel: HashSet<ScheduleEntryId> = HashSet::new();
                while !shut_down.load(Ordering::SeqCst) {
                    // cancel requests take precedence
                    while let Some(entry_id) = cancel_queue.pop() {
                        trace!(
                            "{}: cancelling scheduler entry with id {:?};",
                            name,
                            entry_id
                        );
                        let _ = entries_to_cancel.insert(entry_id);
                    }
                    if let Some(entry) = schedule_queue.pop() {
                        trace!("{}: scheduling entry; {:?};", name, entry);
                        if entries.insert(entry) {
                            entry_count.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    if let Some(entry) = entries.iter().next().cloned() {
                        let now = Instant::now();
                        // time to execute a callback?
                        if now.ge(&entry.start) {
                            entries.remove(&entry);
                            // entry still relevant ?
                            if !entries_to_cancel.contains(&entry.id) {
                                trace!("{}: executing scheduler entry; {:?}", name, entry);
                                let cb = Arc::clone(&entry.callback);
                                cb();
                                if let Some(interval) = entry.interval {
                                    // add back
                                    let updated_entry = ScheduleEntry {
                                        start: Instant::now().add(interval),
                                        interval: entry.interval,
                                        callback: entry.callback,
                                        name: entry.name,
                                        id: entry.id,
                                    };
                                    entries.insert(updated_entry);
                                }
                            } else {
                                // not executing and not scheduling a new entry
                                trace!("{}: cancelling scheduler entry; {:?}", name, entry);

                                if entries_to_cancel.remove(&entry.id) {
                                    entry_count.fetch_sub(1, Ordering::SeqCst);
                                }
                            }
                        } else {
                            // park until the nearest time when we need to execute a function
                            let timeout_dur = entry.start.sub(now);
                            trace!("{}: parking scheduler for {:?}", name, timeout_dur);
                            *status.write().unwrap() = SchedulerStatus::ParkedTimeout;
                            thread::park_timeout(timeout_dur);
                            *status.write().unwrap() = SchedulerStatus::Active;
                        }
                    } else {
                        // there's no function to execute, so park indefinitely instead of spinning idly
                        trace!("{}: parking scheduler until being un-parked", name);
                        *status.write().unwrap() = SchedulerStatus::Parked;
                        thread::park();
                        *status.write().unwrap() = SchedulerStatus::Active;
                    }
                }
            })
            .unwrap();
        Scheduler {
            shutdown: shutdown_clone,
            thread_handle,
            schedule_queue: schedule_queue_clone,
            cancel_queue: cancel_queue_clone,
            name: name_clone,
            status: status_clone,
            entry_count: entry_count_clone,
        }
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.thread_handle.thread().unpark();
    }
}
