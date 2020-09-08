use std::thread;
use std::time::Duration;

use mio::{Events, Interest, Poll, Token, Waker};

use crate::{expect_events, TestEvent};
use mio_misc::{channel, poll, queue::NotificationQueue, NotificationId};
use std::sync::Arc;

use mio::net::{TcpListener, TcpStream};
use std::net::SocketAddr;

const NO_EVENTS_TEXT: &str = "there should be no events";
const EVENTS_EXIST_TEXT: &str = "there should be events";

#[test]
pub fn test_poll() {
    let mut poll = poll::Poll::with_capacity(100).unwrap();

    let events = poll.wait(Duration::from_millis(200)).unwrap();

    assert!(events.is_empty());

    let waker = Waker::new(poll.registry(), Token(0)).unwrap();

    let _ = waker.wake();

    let events = poll.wait(Duration::from_millis(200)).unwrap();

    assert!(!events.is_empty());
    let event = events.iter().next().unwrap();
    assert_eq!(event.token(), Token(0));

    // Create the listener
    let address: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut listener = TcpListener::bind(address).unwrap();
    poll.registry()
        .register(
            &mut listener,
            Token(1),
            Interest::READABLE.add(Interest::WRITABLE),
        )
        .unwrap();

    // Connect a TCP socket to listener
    let mut socket = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    // Register the socket
    poll.registry()
        .register(
            &mut socket,
            Token(2),
            Interest::READABLE.add(Interest::WRITABLE),
        )
        .unwrap();

    // Sleep a bit to ensure it arrives at dest
    thread::sleep(Duration::from_secs(2));

    let _ = waker.wake();

    expect_events(
        &mut poll,
        2,
        vec![
            TestEvent {
                interest: Interest::READABLE,
                token: Token(0),
            },
            TestEvent {
                interest: Interest::READABLE,
                token: Token(1),
            },
            TestEvent {
                interest: Interest::WRITABLE,
                token: Token(2),
            },
        ],
    );
}

#[test]
pub fn test_poll_channel() {
    let poll_timeout_duration = Duration::from_millis(300);
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(1024);
    let waker_token = Token(2);
    let waker = Arc::new(Waker::new(poll.registry(), waker_token).unwrap());

    let queue = Arc::new(NotificationQueue::new(waker));
    let channel_notifier = Arc::clone(&queue);

    let channel_1_id = NotificationId::gen_next();
    let (tx1, rx1) = channel::channel(channel_notifier, channel_1_id);

    let channel_notifier = Arc::clone(&queue);
    let channel_2_id = NotificationId::gen_next();
    let (tx2, rx2) = channel::channel(channel_notifier, channel_2_id);

    poll.poll(&mut events, Some(poll_timeout_duration)).unwrap();
    assert!(events.is_empty(), NO_EVENTS_TEXT);

    // Push the value
    tx1.send("hello").unwrap();
    tx2.send("world").unwrap();

    // Polling will contain the event
    poll.poll(&mut events, Some(poll_timeout_duration)).unwrap();

    let event = events.iter().next().unwrap();
    assert_eq!(event.token(), waker_token);
    assert!(event.is_readable());
    assert_eq!(queue.pop().unwrap(), channel_1_id);
    assert_eq!(queue.pop().unwrap(), channel_2_id);

    // Poll again and there should be no events
    poll.poll(&mut events, Some(poll_timeout_duration)).unwrap();
    assert!(events.is_empty(), NO_EVENTS_TEXT);
    assert!(queue.pop().is_none(), "there should be no notifications");

    // Read the values
    assert_eq!("hello", rx1.try_recv().unwrap());
    assert_eq!("world", rx2.try_recv().unwrap());

    poll.poll(&mut events, Some(poll_timeout_duration)).unwrap();
    assert!(events.is_empty(), NO_EVENTS_TEXT);

    // Push values
    tx2.send("meow").unwrap();
    tx1.send("woof").unwrap();

    poll.poll(&mut events, Some(poll_timeout_duration)).unwrap();
    assert!(!events.is_empty(), EVENTS_EXIST_TEXT);

    let event = events.iter().next().unwrap();
    assert_eq!(event.token(), waker_token);
    assert!(event.is_readable());
    assert_eq!(queue.pop().unwrap(), channel_2_id);
    assert_eq!(queue.pop().unwrap(), channel_1_id);
}

#[test]
pub fn test_poll_channel_crossbeam() {
    let poll_timeout_duration = Duration::from_millis(300);
    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(1024);
    let waker_token = Token(2);
    let waker = Arc::new(Waker::new(poll.registry(), waker_token).unwrap());

    let queue = Arc::new(NotificationQueue::new(waker));
    let channel_notifier = Arc::clone(&queue);

    let channel_1_id = NotificationId::gen_next();
    let (tx1, rx1) = channel::crossbeam_channel_unbounded(channel_notifier, channel_1_id);

    let channel_notifier = Arc::clone(&queue);
    let channel_2_id = NotificationId::gen_next();
    let (tx2, rx2) = channel::crossbeam_channel_unbounded(channel_notifier, channel_2_id);

    poll.poll(&mut events, Some(poll_timeout_duration)).unwrap();
    assert!(events.is_empty(), NO_EVENTS_TEXT);

    // Push the value
    tx1.send("hello").unwrap();
    tx2.send("world").unwrap();

    // Polling will contain the event
    poll.poll(&mut events, Some(poll_timeout_duration)).unwrap();

    let event = events.iter().next().unwrap();
    assert_eq!(event.token(), waker_token);
    assert!(event.is_readable());
    assert_eq!(queue.pop().unwrap(), channel_1_id);
    assert_eq!(queue.pop().unwrap(), channel_2_id);

    // Poll again and there should be no events
    poll.poll(&mut events, Some(poll_timeout_duration)).unwrap();
    assert!(events.is_empty(), NO_EVENTS_TEXT);
    assert!(queue.pop().is_none(), "there should be no notifications");

    // Read the values
    assert_eq!("hello", rx1.try_recv().unwrap());
    assert_eq!("world", rx2.try_recv().unwrap());

    poll.poll(&mut events, Some(poll_timeout_duration)).unwrap();
    assert!(events.is_empty(), NO_EVENTS_TEXT);

    // Push values
    tx2.send("meow").unwrap();
    tx1.send("woof").unwrap();

    poll.poll(&mut events, Some(poll_timeout_duration)).unwrap();
    assert!(!events.is_empty(), EVENTS_EXIST_TEXT);

    let event = events.iter().next().unwrap();
    assert_eq!(event.token(), waker_token);
    assert!(event.is_readable());
    assert_eq!(queue.pop().unwrap(), channel_2_id);
    assert_eq!(queue.pop().unwrap(), channel_1_id);
}

#[test]
pub fn test_sending_from_other_thread_while_polling() {
    let iterations: usize = 20;
    let threads: usize = 5;

    // Make sure to run multiple times
    let mut poll = poll::Poll::with_capacity(20).unwrap();

    let waker = Arc::new(Waker::new(poll.registry(), Token(0)).unwrap());
    let queue = Arc::new(NotificationQueue::new(waker));

    for _ in 0..iterations {
        let channel_notifier = Arc::clone(&queue);
        let (tx, rx) = channel::channel(channel_notifier, NotificationId::gen_next());

        for _ in 0..threads {
            let tx = tx.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(50));
                tx.send("ping").unwrap();
            });
        }

        let mut recv = 0;
        while recv < threads {
            let events = poll.wait(Duration::from_secs(2)).unwrap();
            assert_eq!(events.iter().next().unwrap().token(), Token(0));
            while let Ok(_) = rx.try_recv() {
                recv += 1;
            }
        }
    }
}

#[test]
pub fn test_dropping_receive_before_poll() {
    let mut poll = poll::Poll::with_capacity(10).unwrap();
    let waker = Arc::new(Waker::new(poll.registry(), Token(0)).unwrap());
    let queue = Arc::new(NotificationQueue::new(waker));
    let channel_notifier = Arc::clone(&queue);
    let (tx, rx) = channel::channel(channel_notifier, NotificationId::gen_next());

    drop(rx);
    assert!(
        tx.send("hello").is_err(),
        "sending should fail after dropping the receiving end"
    );

    let events = poll.wait(Duration::from_millis(300)).unwrap();
    assert!(events.is_empty(), NO_EVENTS_TEXT);
}

#[test]
pub fn test_mixing_channel_with_socket() {
    let mut poll = poll::Poll::with_capacity(20).unwrap();
    let waker = Arc::new(Waker::new(poll.registry(), Token(0)).unwrap());
    let queue = Arc::new(NotificationQueue::new(waker));
    let (tx, _rx) = channel::channel(queue, NotificationId::gen_next());

    let address: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut listener = TcpListener::bind(address).unwrap();
    poll.registry()
        .register(
            &mut listener,
            Token(1),
            Interest::READABLE.add(Interest::WRITABLE),
        )
        .unwrap();

    // Push a value onto the channel
    tx.send("hello").unwrap();

    // Connect a TCP socket to listener
    let mut socket = TcpStream::connect(listener.local_addr().unwrap()).unwrap();

    // Register the socket
    poll.registry()
        .register(
            &mut socket,
            Token(2),
            Interest::READABLE.add(Interest::WRITABLE),
        )
        .unwrap();

    // Sleep a bit to ensure it arrives at dest
    thread::sleep(Duration::from_millis(250));

    expect_events(
        &mut poll,
        2,
        vec![
            TestEvent {
                interest: Interest::READABLE,
                token: Token(0),
            },
            TestEvent {
                interest: Interest::READABLE,
                token: Token(1),
            },
        ],
    );
}
