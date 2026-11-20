use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::*;

#[test]
fn answers_come_back_in_the_order_of_what_was_asked() {
  // The first asked takes the longest, so it is the last to finish.
  let doubled = in_order(Some(2), (1..=12).collect(), |number: u64| {
    std::thread::sleep(Duration::from_millis(12 - number));
    number * 2
  });
  assert_eq!(
    doubled,
    (1..=12).map(|number| number * 2).collect::<Vec<u64>>()
  );
}

#[test]
fn no_more_are_read_at_once_than_the_cap_allows() {
  let (reading, most) = (AtomicUsize::new(0), AtomicUsize::new(0));
  in_order(Some(2), (0..16).collect(), |_: u32| {
    most.fetch_max(reading.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(5));
    reading.fetch_sub(1, Ordering::SeqCst);
  });
  let most = most.load(Ordering::SeqCst);
  assert!((1..=2).contains(&most), "{most} were being read at once");
}
