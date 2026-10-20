use worldwright_core::{
    scheduler::{ScheduleError, Scheduler},
    time::Tick,
};

#[test]
fn queue_orders_deadlines_and_equal_deadlines_fifo() {
    let mut queue = Scheduler::new(3).unwrap();
    queue.schedule(Tick::from_millis(20), "later").unwrap();
    queue.schedule(Tick::from_millis(10), "first").unwrap();
    queue.schedule(Tick::from_millis(10), "second").unwrap();
    assert_eq!(queue.pop_due(Tick::from_millis(9)).unwrap(), None);
    assert_eq!(
        queue
            .pop_due(Tick::from_millis(10))
            .unwrap()
            .unwrap()
            .payload,
        "first"
    );
    assert_eq!(
        queue
            .pop_due(Tick::from_millis(10))
            .unwrap()
            .unwrap()
            .payload,
        "second"
    );
    assert_eq!(queue.pop_due(Tick::from_millis(19)).unwrap(), None);
    assert_eq!(
        queue
            .pop_due(Tick::from_millis(20))
            .unwrap()
            .unwrap()
            .payload,
        "later"
    );
    assert!(queue.is_empty());
}

#[test]
fn capacity_and_cancel_are_explicit() {
    assert!(matches!(
        Scheduler::<()>::new(0),
        Err(ScheduleError::InvalidCapacity)
    ));
    let mut queue = Scheduler::new(1).unwrap();
    let token = queue.schedule(Tick::ZERO, "one").unwrap();
    assert_eq!(queue.schedule(Tick::ZERO, "two"), Err(ScheduleError::Full));
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.cancel(token).unwrap().payload, "one");
    assert_eq!(queue.cancel(token), None);
    let replacement = queue.schedule(Tick::ZERO, "two").unwrap();
    assert_ne!(token, replacement);
    assert_eq!(queue.cancel(token), None);
    assert_eq!(queue.len(), 1);
}

#[test]
fn backwards_poll_is_rejected_without_removing_work() {
    let mut queue = Scheduler::new(1).unwrap();
    queue.schedule(Tick::from_millis(20), "future").unwrap();
    queue.pop_due(Tick::from_millis(10)).unwrap();
    assert_eq!(
        queue.pop_due(Tick::ZERO),
        Err(ScheduleError::ClockWentBackwards)
    );
    assert_eq!(queue.len(), 1);
    assert_eq!(
        queue
            .pop_due(Tick::from_millis(20))
            .unwrap()
            .unwrap()
            .payload,
        "future"
    );
}

#[test]
fn overdue_insert_is_due_without_resetting_clock() {
    let mut queue = Scheduler::new(1).unwrap();
    queue.pop_due(Tick::from_millis(10)).unwrap();
    queue.schedule(Tick::ZERO, "overdue").unwrap();
    assert_eq!(
        queue
            .pop_due(Tick::from_millis(10))
            .unwrap()
            .unwrap()
            .payload,
        "overdue"
    );
}
