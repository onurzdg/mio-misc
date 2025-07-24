use mio::{Token, Waker};

use mio_misc::scheduler::{NotificationScheduler, ScheduleEntry, SchedulerStatus};
use mio_misc::{poll, queue::NotificationQueue, scheduler::Scheduler, NotificationId};
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::HashMap;
use std::ops::{Add, Sub};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Barrier, Mutex, RwLock};
use std::thread;
use std::time::Duration;

#[test]
pub fn test_notification_scheduler() {
    let mut poll = poll::Poll::with_capacity(100).unwrap();
    let waker_token = Token(2);
    let waker = Arc::new(Waker::new(poll.registry(), waker_token).unwrap());
    let queue = Arc::new(NotificationQueue::new(waker));

    let notifier = Arc::clone(&queue);
    let id1 = NotificationId::gen_next();

    let scheduler = NotificationScheduler::new(notifier, Arc::new(Scheduler::default()));
    scheduler.notify_with_fixed_interval(
        id1,
        Duration::from_millis(1),
        None,
        Some("notification-name-1".into()),
    );

    let id2 = NotificationId::gen_next();
    scheduler.notify_with_fixed_interval(
        id2,
        Duration::from_millis(1),
        None,
        Some("notification-name-2".into()),
    );

    thread::sleep(Duration::from_millis(33));

    let events = poll.poll(Duration::from_millis(1)).unwrap();
    assert!(!events.is_empty());
    assert_eq!(events.iter().next().unwrap().token(), waker_token);
    assert!(queue.len() >= 4);

    let mut map: HashMap<NotificationId, u32> = [(id1, 2), (id2, 2)].iter().cloned().collect();

    let update_counts = |_| {
        let _ = map.entry(queue.pop().unwrap()).and_modify(|v| *v -= 1);
    };

    (1..=4).for_each(update_counts);

    // make sure we at least we get two notifications of each id
    assert!(map.iter().all(|(_, &count)| count == 0));
}

#[test]
pub fn test_scheduler_with_fixed_interval() {
    let atomic_int = Arc::new(AtomicU32::new(0));
    let interval = Duration::from_millis(2);
    let atomic_int_2 = Arc::clone(&atomic_int);
    let scheduler = Scheduler::default();
    let entry =
        ScheduleEntry::with_interval(interval, None, Some("increment-int".into()), move || {
            atomic_int_2.fetch_add(1, Ordering::SeqCst);
        });
    let entry_id = entry.id;
    scheduler.schedule(entry);

    thread::sleep(interval.add(Duration::from_millis(15)));
    scheduler.cancel(entry_id);

    let called_times = atomic_int.load(Ordering::SeqCst);
    assert!(called_times >= 2);

    thread::sleep(Duration::from_millis(20));
    assert_eq!(scheduler.status(), SchedulerStatus::Parked);
}

#[test]
pub fn test_scheduler_with_fixed_interval_cancel() {
    let atomic_int = Arc::new(AtomicU32::new(0));
    let atomic_int_clone = Arc::clone(&atomic_int);
    let interval = Duration::from_millis(500);
    let entry =
        ScheduleEntry::with_interval(interval, Duration::from_millis(200), None, move || {
            atomic_int_clone.fetch_add(1, Ordering::SeqCst);
        });
    let entry_id_1 = entry.id;
    let scheduler = Scheduler::default();
    scheduler.schedule(entry);

    scheduler.cancel(entry_id_1);
    let called_times = atomic_int.load(Ordering::SeqCst);
    assert_eq!(called_times, 0);

    let atomic_int_clone2 = Arc::clone(&atomic_int);
    let entry2 = ScheduleEntry::with_interval(
        Duration::from_millis(800),
        Duration::from_millis(200),
        None,
        move || {
            atomic_int_clone2.fetch_add(1, Ordering::SeqCst);
        },
    );
    let entry_id_2 = entry2.id;
    scheduler.schedule(entry2);

    // wait for the entry to be scheduled
    thread::sleep(Duration::from_millis(50));
    scheduler.cancel(entry_id_2);
    let possible_states = vec![SchedulerStatus::ParkedTimeout, SchedulerStatus::Active];
    assert!(possible_states.contains(&scheduler.status()));

    let called_times = atomic_int.load(Ordering::SeqCst);
    assert_eq!(called_times, 0);

    thread::sleep(Duration::from_millis(400));
    assert_eq!(scheduler.status(), SchedulerStatus::Parked);
}

#[test]
pub fn test_scheduler_one_time() {
    let atomic_int = Arc::new(AtomicU32::new(0));
    let atomic_int_clone = Arc::clone(&atomic_int);

    let delay = Duration::from_millis(20);
    let scheduler = Scheduler::default();
    let possible_states = vec![SchedulerStatus::Parked, SchedulerStatus::Active];
    assert!(possible_states.contains(&scheduler.status()));

    let entry = ScheduleEntry::one_time(delay, None, move || {
        atomic_int_clone.fetch_add(1, Ordering::SeqCst);
    });
    scheduler.schedule(entry);
    // wait to be able to test with certainty that it won't be executed multiple times
    thread::sleep(delay.add(Duration::from_millis(70)));
    let called_times = atomic_int.load(Ordering::SeqCst);
    assert_eq!(called_times, 1);

    let atomic_int_clone_2 = Arc::clone(&atomic_int);
    let entry2 = ScheduleEntry::one_time(Duration::from_millis(200), None, move || {
        atomic_int_clone_2.fetch_add(1, Ordering::SeqCst);
    });
    scheduler.schedule(entry2);
    // wait for entry to be scheduled
    thread::sleep(Duration::from_millis(100));
    assert_eq!(scheduler.status(), SchedulerStatus::ParkedTimeout);

    // wait just enough for the entry to be executed
    thread::sleep(Duration::from_millis(140));
    let called_times = atomic_int.load(Ordering::SeqCst);
    assert_eq!(called_times, 2);
    assert_eq!(scheduler.status(), SchedulerStatus::Parked);

    let atomic_int_clone_3 = Arc::clone(&atomic_int);
    let entry3 = ScheduleEntry::one_time(Duration::from_millis(110), None, move || {
        atomic_int_clone_3.fetch_add(1, Ordering::SeqCst);
    });
    scheduler.schedule(entry3);
    // wait just enough for the entry to be in the queue
    thread::sleep(Duration::from_millis(50));
    assert_eq!(scheduler.status(), SchedulerStatus::ParkedTimeout);
}

#[test]
pub fn test_scheduler_one_time_cancel() {
    let atomic_int = Arc::new(AtomicU32::new(0));
    let atomic_int_clone = Arc::clone(&atomic_int);
    let scheduler = Scheduler::default();
    let delay = Duration::from_millis(70);
    let entry = ScheduleEntry::one_time(delay, None, move || {
        atomic_int_clone.fetch_add(1, Ordering::SeqCst);
    });
    let entry_id_1 = entry.id;
    scheduler.schedule(entry);

    // wait less than the delay assigned to entry so that we prevent the function from being executed
    thread::sleep(delay.sub(Duration::from_millis(60)));
    scheduler.cancel(entry_id_1);
    let called_times = atomic_int.load(Ordering::SeqCst);
    assert_eq!(called_times, 0);

    let atomic_int_clone2 = Arc::clone(&atomic_int);
    let entry2 = ScheduleEntry::one_time(delay, None, move || {
        atomic_int_clone2.fetch_add(1, Ordering::SeqCst);
    });
    let entry_id_2 = entry2.id;
    scheduler.schedule(entry2);

    // wait more than the delay assigned to schedule entry so that cancel gets called after entry execution
    thread::sleep(delay.add(Duration::from_millis(35)));
    scheduler.cancel(entry_id_2);
    let called_times = atomic_int.load(Ordering::SeqCst);
    assert_eq!(called_times, 1);

    // verifying that no execution happens after the previous one
    thread::sleep(delay.add(Duration::from_millis(35)));
    let called_times = atomic_int.load(Ordering::SeqCst);
    assert_eq!(called_times, 1);
    assert_eq!(scheduler.status(), SchedulerStatus::Parked);
}

#[test]
pub fn stress_test_execution() {
    let scheduler = Scheduler::default();
    let mut rng = rand::rng();

    let map: Arc<RwLock<HashMap<u32, Mutex<u32>>>> = Arc::new(RwLock::new(HashMap::new()));
    let total_entries = 50;
    let mut entry_ids = Vec::with_capacity(total_entries);

    for idx in 1..=(total_entries as u32) {
        let interval = Duration::from_millis(rng.random_range(1..101));
        let map_clone = Arc::clone(&map);
        let entry =
            ScheduleEntry::with_interval(interval, None, Some("increment-int".into()), move || {
                let map_read_guard = map_clone.read().expect("RwLock poisoned");
                if let Some(entry) = map_read_guard.get(&idx) {
                    let mut elem = entry.lock().expect("Mutex poisoned");
                    *elem += 1;
                } else {
                    drop(map_read_guard); // drop read lock
                    let mut map_write_guard = map_clone.write().expect("RwLock poisoned");
                    map_write_guard.entry(idx).or_insert_with(|| Mutex::new(1));
                }
            });
        entry_ids.push(entry.id);
        scheduler.schedule(entry);
    }

    // wait for entries to be scheduled and run
    thread::sleep(Duration::from_secs(10));

    assert_eq!(
        scheduler.entry_count(),
        total_entries as u32,
        "entry count should match after initial scheduling of entries"
    );
    assert_eq!(
        entry_ids.len(),
        total_entries as usize,
        "entry id count should match after initial scheduling of entries"
    );
    assert!(
        map.read()
            .unwrap()
            .iter()
            .all(|(_, count)| *(count.lock().unwrap()) > 0),
        "all entries should have be executed"
    );

    entry_ids.shuffle(&mut rng);
    let removed_entries = entry_ids.split_off(total_entries / 2);
    for removed_entry in removed_entries {
        scheduler.cancel(removed_entry);
    }

    // wait for entries to be cancelled
    thread::sleep(Duration::from_secs(5));

    // remove all entries from map
    map.write().unwrap().clear();

    // wait for remaining entries to keep executing
    thread::sleep(Duration::from_secs(10));

    assert_eq!(
        scheduler.entry_count(),
        (total_entries / 2) as u32,
        "new entry count should match after cancellations"
    );
    assert!(
        map.read()
            .unwrap()
            .iter()
            .all(|(_, count)| *(count.lock().unwrap()) > 0),
        "all remaining entries should have continued to execute"
    );
}

#[test]
pub fn stress_test_execution_with_multiple_threads() {
    let scheduler = Arc::new(Scheduler::default());
    let mut rng = rand::rng();
    let barrier = Arc::new(Barrier::new(10));

    let map: Arc<RwLock<HashMap<u32, Mutex<u32>>>> = Arc::new(RwLock::new(HashMap::new()));
    let total_entries = 10;
    let mut entry_ids = Vec::with_capacity(total_entries);
    let mut thread_handles = Vec::with_capacity(total_entries);

    for idx in 1..=(total_entries as u32) {
        let interval = Duration::from_millis(rng.random_range(1..101));
        let map_clone = Arc::clone(&map);
        let entry =
            ScheduleEntry::with_interval(interval, None, Some("increment-int".into()), move || {
                let map_read_guard = map_clone.read().expect("RwLock poisoned");
                if let Some(entry) = map_read_guard.get(&idx) {
                    let mut elem = entry.lock().expect("Mutex poisoned");
                    *elem += 1;
                } else {
                    drop(map_read_guard); // drop read lock
                    let mut map_write_guard = map_clone.write().expect("RwLock poisoned");
                    map_write_guard.entry(idx).or_insert_with(|| Mutex::new(1));
                }
            });
        entry_ids.push(entry.id);
        let barrier_clone = Arc::clone(&barrier);
        let scheduler_clone = Arc::clone(&scheduler);
        thread_handles.push(thread::spawn(move || {
            barrier_clone.wait(); // wait for all threads to reach the barrier
            scheduler_clone.schedule(entry);
        }));
    }

    // wait for threads to finish and entries to be scheduled and run
    for handle in thread_handles {
        handle.join().unwrap();
    }
    thread::sleep(Duration::from_secs(10));

    assert_eq!(
        scheduler.entry_count(),
        total_entries as u32,
        "entry count should match after initial scheduling of entries"
    );
    assert_eq!(
        entry_ids.len(),
        total_entries as usize,
        "entry id count should match after initial scheduling of entries"
    );
    assert!(
        map.read()
            .unwrap()
            .iter()
            .all(|(_, count)| *(count.lock().unwrap()) > 0),
        "all entries should have be executed"
    );

    entry_ids.shuffle(&mut rng);
    let removed_entries = entry_ids.split_off(total_entries / 2);
    for removed_entry in removed_entries {
        scheduler.cancel(removed_entry);
    }

    // wait for entries to be cancelled
    thread::sleep(Duration::from_secs(5));

    // remove all entries from map
    map.write().unwrap().clear();

    // wait for remaining entries to keep executing
    thread::sleep(Duration::from_secs(10));

    assert_eq!(
        scheduler.entry_count(),
        (total_entries / 2) as u32,
        "new entry count should match after cancellations"
    );
    assert!(
        map.read()
            .unwrap()
            .iter()
            .all(|(_, count)| *(count.lock().unwrap()) > 0),
        "all remaining entries should have continued to execute"
    );
}
