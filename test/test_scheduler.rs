use mio::{Token, Waker};

use mio_misc::{scheduler, poll, scheduler::Scheduler, NotificationId, queue::NotificationQueue};
use std::sync::atomic::{Ordering, AtomicU32};
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use std::thread;
use std::ops::Add;

#[test]
pub fn test_notification_scheduler() {
    let mut poll = poll::Poll::with_capacity(100).unwrap();
    let waker_token = Token(2);
    let waker = Arc::new(Waker::new(poll.registry(), waker_token).unwrap());
    let queue = Arc::new(NotificationQueue::new(waker));

    let id1 = NotificationId::gen_next();
    let scheduler1 =
        scheduler::NotificationScheduler::notify_with_fixed_interval(
            id1, Arc::clone(&queue), Duration::from_millis(1), None);

    let id2 = NotificationId::gen_next();
    let scheduler2 =
        scheduler::NotificationScheduler::notify_with_fixed_interval(
            id2, Arc::clone(&queue), Duration::from_millis(1), None);

    thread::sleep(Duration::from_millis(3));

    let events = poll.wait(Duration::from_millis(1)).unwrap();
    assert!(!events.is_empty());
    assert_eq!(events.iter().next().unwrap().token(), waker_token);
    assert!(queue.len() >= 4);

    let mut map: HashMap<NotificationId, u32> = [(id1, 2), (id2, 2)]
        .iter().cloned().collect();

    let update_counts = |_|
        { let _ = map.entry(queue.pop().unwrap())
            .and_modify(|v|*v -= 1);
        };

    (1..=4).for_each(update_counts);

    assert!(map.iter().all(|(_, &count)| count == 0));

    assert!(scheduler1.cancel().is_ok());
    assert!(scheduler2.cancel().is_ok());
}

#[test]
pub fn test_scheduler_with_fixed_interval() {
    let atomic_int = Arc::new(AtomicU32::new(0));
    let interval = Duration::from_millis(2);
    let atomic_int_2 = Arc::clone(&atomic_int);
    let sh = Scheduler::schedule_with_fixed_interval(interval,
                                                     None,
                                                     move || { atomic_int_2.fetch_add(1, Ordering::SeqCst); });
    thread::sleep(interval.add(Duration::from_millis(5)));
    assert!(sh.cancel().is_ok());

    let called_times = atomic_int.load(Ordering::SeqCst);
    assert!(called_times >= 2);
}

#[test]
pub fn test_scheduler_with_fixed_interval_cancel() {
    let atomic_int = Arc::new(AtomicU32::new(0));
    let interval = Duration::from_millis(5);
    let atomic_int_2 = Arc::clone(&atomic_int);
    let sh = Scheduler::schedule_with_fixed_interval(interval,
                                                     None,
                                                     move || { atomic_int_2.fetch_add(1, Ordering::SeqCst); });
    assert!(sh.cancel().is_ok());
    let called_times = atomic_int.load(Ordering::SeqCst);
    assert_eq!(called_times, 0);

    let atomic_int_3 = Arc::clone(&atomic_int);
    let sh = Scheduler::schedule_with_fixed_interval(interval,
                                                     Duration::from_secs(2),
                                                     move || { atomic_int_3.fetch_add(1, Ordering::SeqCst); });

    thread::sleep(Duration::from_millis(500));
    assert!(sh.cancel().is_ok());
    let called_times = atomic_int.load(Ordering::SeqCst);
    assert_eq!(called_times, 0);
}

#[test]
pub fn test_scheduler_one_time() {
    let atomic_int = Arc::new(AtomicU32::new(0));
    let interval = Duration::from_millis(2);
    let atomic_int_2 = Arc::clone(&atomic_int);
    let sh = Scheduler::schedule_one_time(interval,
                                          move || { atomic_int_2.fetch_add(1, Ordering::SeqCst); });
    thread::sleep(interval.add(Duration::from_millis(4)));
    assert!(sh.cancel().is_ok());

    let called_times = atomic_int.load(Ordering::SeqCst);
    assert_eq!(called_times, 1);
}

#[test]
pub fn test_cancel_scheduler_one_time() {
    let atomic_int = Arc::new(AtomicU32::new(0));
    let atomic_int_2 = Arc::clone(&atomic_int);
    let sh = Scheduler::schedule_one_time(Duration::from_millis(40),
                                          move || { atomic_int_2.fetch_add(1, Ordering::SeqCst); });

    thread::sleep(Duration::from_millis(15));
    assert!(sh.cancel().is_ok());
    let called_times = atomic_int.load(Ordering::SeqCst);
    assert_eq!(called_times, 0);
}