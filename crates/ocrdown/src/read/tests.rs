use super::*;

#[test]
fn pixel_boxes_map_onto_the_destination_rectangle() {
  let words = vec![OcrWord {
    text: "Hi".into(),
    left: 0,
    top: 0,
    right: 50,
    bottom: 10,
    confidence: 87.5,
  }];
  // 100×100 px image painted on [10, 20, 110, 220]: 1 px = 1 pt wide, 2 pt tall.
  let mapped = place_words(words, 100, 100, [10.0, 20.0, 110.0, 220.0]);
  let w = &mapped[0];
  assert_eq!((w.x0, w.x1), (10.0, 60.0));
  assert_eq!((w.y0, w.y1), (200.0, 220.0));
  assert_eq!(w.confidence, Some(0.875));
  assert_eq!(w.baseline, 200.0);
}

/// What a pass gives when it reads this well: forty words, each this sure.
fn pass(confidence: f32) -> Pass {
  let word = OcrWord {
    text: "word".into(),
    left: 0,
    top: 0,
    right: 40,
    bottom: 12,
    confidence,
  };
  (vec![word; 40], (800, 600))
}

/// The search for the way up, over a picture that reads this well at each turn. With it, the
/// turns that were read, in order, and whether the detector was asked.
fn searched(
  expected: u32,
  reads: impl Fn(u32) -> f32,
  detector: Option<(u32, f32)>,
) -> (Upright, Vec<u32>, bool) {
  let mut read = Vec::new();
  let mut asked = false;
  let found = way_up(
    expected,
    |degrees| {
      read.push(degrees);
      pass(reads(degrees))
    },
    || {
      asked = true;
      detector
    },
  );
  (found, read, asked)
}

#[test]
fn with_no_turn_expected_a_picture_that_reads_well_upright_is_read_once() {
  let (found, read, asked) = searched(0, |_| 96.0, Some((90, 9.0)));
  assert_eq!(read, [0]);
  assert!(!asked, "the detector was asked about a page that read well");
  assert_eq!(
    (found.degrees, found.decided),
    (0, OrientationSource::Upright)
  );
}

#[test]
fn the_turn_a_picture_is_expected_to_need_is_read_first_and_stands_when_it_reads_well() {
  // A page that says it is shown turned, holding a picture kept as it was scanned.
  let well_at = |turn: u32| move |degrees: u32| if degrees == turn { 96.0 } else { 44.0 };
  for turn in [90, 180, 270] {
    let (found, read, asked) = searched(turn, well_at(turn), Some((turn, 9.0)));
    assert_eq!(read, [turn], "only the expected turn is read");
    assert!(
      !asked,
      "the detector was asked though the expected turn read well"
    );
    assert_eq!(
      (found.degrees, found.decided),
      (turn, OrientationSource::Expected)
    );
    assert_eq!(found.size, (800, 600));
  }
}

#[test]
fn an_expected_turn_that_reads_badly_gives_way_to_the_picture_as_it_is() {
  // The page says it is shown turned, and the picture in it is upright already.
  let (found, read, asked) = searched(270, |degrees| if degrees == 0 { 96.0 } else { 44.0 }, None);
  assert_eq!(read, [270, 0]);
  assert!(!asked);
  assert_eq!(
    (found.degrees, found.decided),
    (0, OrientationSource::Upright)
  );
}

#[test]
fn a_turn_already_read_is_not_read_again_when_the_search_comes_to_it() {
  // The expected turn reads fairly, not well, and nothing else reads at all: the detector is
  // asked, names the same turn, and what was read of it already is used.
  let fairly = |degrees: u32| if degrees == 270 { 70.0 } else { 40.0 };
  let (found, read, asked) = searched(270, fairly, Some((270, 9.0)));
  assert_eq!(read, [270, 0]);
  assert!(asked);
  // It is the detector's turn, as it would have been with nothing expected.
  assert_eq!(
    (found.degrees, found.decided),
    (270, OrientationSource::Osd)
  );
  // The same when the detector is silent and the other turns are tried in their order.
  let (found, read, _) = searched(270, fairly, None);
  assert_eq!(read, [270, 0, 90]);
  assert_eq!(
    (found.degrees, found.decided),
    (270, OrientationSource::Retry)
  );
}

#[test]
fn an_expected_turn_that_reads_only_fairly_changes_nothing_of_what_the_search_finds() {
  // A picture with little on it: what was expected reads a little surer than the picture as
  // it is, and neither reads well. The picture as it is stood before anything was expected,
  // and stands now.
  let reads = |degrees: u32| match degrees {
    270 => 72.0,
    0 => 61.0,
    _ => 30.0,
  };
  for detector in [None, Some((90, 1.0))] {
    let (found, read, asked) = searched(270, reads, detector);
    let (unexpected, _, _) = searched(0, reads, detector);
    assert_eq!(read, [270, 0]);
    assert!(asked);
    assert_eq!(
      (found.degrees, found.decided),
      (0, OrientationSource::Upright)
    );
    assert_eq!(found, unexpected);
  }
}

#[test]
fn whatever_is_expected_a_search_that_does_not_stop_at_it_ends_where_it_always_did() {
  // Every way a picture can read at its four turns, from badly to well, against every turn
  // that can be expected and every answer of the detector: unless the expected turn reads
  // convincingly, the turn found and who found it are what they are with nothing expected.
  let grades = [20.0, 58.0, 72.0, 96.0];
  let detectors = [
    None,
    Some((90, 9.0)),
    Some((180, 9.0)),
    Some((270, 9.0)),
    Some((0, 9.0)),
  ];
  for a in grades {
    for b in grades {
      for c in grades {
        for d in grades {
          let reads = move |degrees: u32| match degrees {
            0 => a,
            90 => b,
            180 => c,
            _ => d,
          };
          for detector in detectors {
            let (plain, _, _) = searched(0, reads, detector);
            for expected in [90, 180, 270] {
              let (found, read, _) = searched(expected, reads, detector);
              let mut once = read.clone();
              once.dedup();
              once.sort_unstable();
              once.dedup();
              assert_eq!(once.len(), read.len(), "a turn was read twice: {read:?}");
              if reads(expected) >= 80.0 {
                assert_eq!(read, [expected]);
                assert_eq!(found.decided, OrientationSource::Expected);
              } else {
                assert_eq!(
                  found, plain,
                  "{expected} expected, {a} {b} {c} {d}, {detector:?}"
                );
              }
            }
          }
        }
      }
    }
  }
}

#[test]
fn the_detector_and_then_the_other_turns_decide_when_what_was_expected_was_wrong() {
  let well_at_180 = |degrees: u32| if degrees == 180 { 91.0 } else { 30.0 };
  // The detector names the turn: it is read next, and stands.
  let (found, read, _) = searched(90, well_at_180, Some((180, 9.0)));
  assert_eq!(read, [90, 0, 180]);
  assert_eq!(
    (found.degrees, found.decided),
    (180, OrientationSource::Osd)
  );
  // The detector has nothing to say: the turns not yet read are tried, in their usual order.
  let (found, read, asked) = searched(90, well_at_180, None);
  assert!(asked);
  assert_eq!(read, [90, 0, 270, 180]);
  assert_eq!(
    (found.degrees, found.decided),
    (180, OrientationSource::Retry)
  );
}

#[test]
fn an_expected_turn_that_is_no_quarter_turn_is_no_expectation() {
  for odd in [45, 360, 1] {
    let (found, read, _) = searched(odd, |_| 96.0, None);
    assert_eq!(read, [0], "{odd}");
    assert_eq!(found.decided, OrientationSource::Upright);
  }
  // A turn past a whole one is the turn that is left over.
  let (found, read, _) = searched(
    630,
    |degrees| if degrees == 270 { 96.0 } else { 44.0 },
    None,
  );
  assert_eq!(read, [270]);
  assert_eq!(found.degrees, 270);
}

#[test]
fn a_drawn_page_kept_on_its_side_is_read_in_one_pass_when_its_turn_is_expected() {
  use image::{GrayImage, Luma};

  use crate::testing::inked;

  // Four lines of four words, drawn upright, and then the picture laid on its side: it reads
  // again after a turn of 270.
  let mut upright = GrayImage::from_pixel(700, 420, Luma([255]));
  for line in 0..4 {
    inked(
      &mut upright,
      "TOTAL 42 TOTAL 42",
      5,
      (60, 40 + line * 90),
      0,
    );
  }
  let on_its_side = DynamicImage::ImageLuma8(upright).rotate90();
  let found = recognize_words_upright_expecting(&on_its_side, "eng", Some(200), 270);
  assert_eq!(
    (found.degrees, found.decided),
    (270, OrientationSource::Expected)
  );
  let text: Vec<&str> = found.words.iter().map(|word| word.text.as_str()).collect();
  assert_eq!(text, ["TOTAL", "42", "TOTAL", "42"].repeat(4));
  // And it is what the search with nothing expected ends with, by the longer way round.
  let unexpected = recognize_words_upright(&on_its_side, "eng", Some(200));
  assert_eq!(unexpected.degrees, 270);
  assert_ne!(unexpected.decided, OrientationSource::Expected);
  assert_eq!(unexpected.words, found.words);
}

#[test]
fn a_picture_of_a_word_or_two_does_not_stop_the_search_at_what_was_expected() {
  // A stamp or a mark reads "well" at more than one turn, so what it reads at the expected
  // turn is no reason to stop: the search is made as it is with nothing expected.
  let few = |count: usize| {
    let (words, size) = pass(96.0);
    (words.into_iter().take(count).collect::<Vec<_>>(), size)
  };
  for count in [1, 2, 9] {
    let mut read = Vec::new();
    let found = way_up(
      270,
      |degrees| {
        read.push(degrees);
        few(count)
      },
      || None,
    );
    assert_eq!(read, [270, 0], "{count} words");
    assert_eq!(
      (found.degrees, found.decided),
      (0, OrientationSource::Upright)
    );
  }
  // Ten words that read well are a page's worth of evidence.
  let mut read = Vec::new();
  let found = way_up(
    270,
    |degrees| {
      read.push(degrees);
      few(10)
    },
    || None,
  );
  assert_eq!(read, [270]);
  assert_eq!(found.decided, OrientationSource::Expected);
}

/// A pass over a picture that is mostly numbers which read either way up: so many of `number`
/// at one confidence, and a few other words at another.
fn mostly_numbers(number: &str, sure: f32, others: &[&str], others_sure: f32) -> Pass {
  let word = |text: &str, confidence: f32| OcrWord {
    text: text.into(),
    left: 0,
    top: 0,
    right: 40,
    bottom: 12,
    confidence,
  };
  let mut words: Vec<OcrWord> = others.iter().map(|text| word(text, others_sure)).collect();
  words.extend(std::iter::repeat_n(word(number, sure), 84));
  (words, (800, 600))
}

/// The search over a picture whose grid reads `606` one way up and `909` the other, with a
/// head that reads only the right way up, which is at `right_way` degrees.
fn search_a_grid(expected: u32, right_way: u32) -> (Upright, Vec<u32>) {
  let mut read = Vec::new();
  let found = way_up(
    expected,
    |degrees| {
      read.push(degrees);
      if degrees == right_way {
        // The right way up: the head reads well, and the grid a little less surely.
        mostly_numbers("606", 82.0, &["Invoice", "total", "$69.00"], 93.0)
      } else if degrees == (right_way + 180) % 360 {
        // Upside down: the grid reads better than it does the right way up, and the head
        // comes out as three sure-of-nothing scraps.
        mostly_numbers("909", 97.0, &["SDIOAU]", "JE}OL", "00'69$"], 24.0)
      } else {
        pass(31.0)
      }
    },
    || None,
  );
  (found, read)
}

#[test]
fn a_word_reads_both_ways_up_when_every_mark_in_it_does() {
  // Marks that are no letter and no digit are passed over: a number is as much a number
  // upside down with a currency mark at its other end.
  for word in [
    "606", "909", "1,000.00", "0.00", "88.10", "$69.00", "NOON", "SWIMS", "pod", "x", "100",
  ] {
    assert!(reads_both_ways(word), "{word}");
  }
  for word in [
    "Invoice",
    "total",
    "252",
    "2019",
    "12/05/2014",
    "QTY",
    "USD",
    "TOTAL",
  ] {
    assert!(!reads_both_ways(word), "{word}");
  }
  // Marks alone are not a word either way.
  assert!(!reads_both_ways("--"));
  assert!(!reads_both_ways(""));
}

#[test]
fn a_page_of_numbers_that_read_either_way_up_is_not_taken_on_its_first_reading() {
  // Read upside down first, because that was expected of it, and it reads better that way
  // than the right way up. Its other way up is read too, and the head decides.
  let (found, read) = search_a_grid(180, 0);
  assert_eq!(read, [180, 0]);
  assert_eq!(
    (found.degrees, found.decided),
    (0, OrientationSource::OneWayWords)
  );
  assert!(found.words.iter().any(|word| word.text == "$69.00"));
  // The same with nothing expected of a picture kept upside down.
  let (found, read) = search_a_grid(0, 180);
  assert_eq!(read, [0, 180]);
  assert_eq!(
    (found.degrees, found.decided),
    (180, OrientationSource::OneWayWords)
  );
  // And on its side, when the turn expected of it is a half turn out.
  let (found, read) = search_a_grid(90, 270);
  assert_eq!(read, [90, 270]);
  assert_eq!(found.degrees, 270);
}

#[test]
fn a_first_reading_that_is_the_right_way_up_stands_after_its_other_way_is_read() {
  // The price of the check: one more reading, and nothing else changes.
  let (found, read) = search_a_grid(0, 0);
  assert_eq!(read, [0, 180]);
  assert_eq!(
    (found.degrees, found.decided),
    (0, OrientationSource::Upright)
  );
  let (found, read) = search_a_grid(270, 270);
  assert_eq!(read, [270, 90]);
  assert_eq!(
    (found.degrees, found.decided),
    (270, OrientationSource::Expected)
  );
}

#[test]
fn a_page_with_nothing_on_it_that_reads_one_way_only_keeps_its_first_reading() {
  // A grid and no head: nothing can say which way up it is, so nothing overturns the first.
  let mut read = Vec::new();
  let found = way_up(
    0,
    |degrees| {
      read.push(degrees);
      match degrees {
        0 => mostly_numbers("909", 97.0, &[], 0.0),
        _ => mostly_numbers("606", 82.0, &[], 0.0),
      }
    },
    || None,
  );
  assert_eq!(read, [0, 180]);
  assert_eq!(
    (found.degrees, found.decided),
    (0, OrientationSource::Upright)
  );
}

#[test]
fn an_ordinary_page_is_not_read_a_second_time() {
  // A few numbers among words is what every invoice is: under half, and the check is not made.
  let mut read = Vec::new();
  let found = way_up(
    0,
    |degrees| {
      read.push(degrees);
      let (mut words, size) = pass(94.0);
      let (numbers, _) = mostly_numbers("1,000.00", 94.0, &[], 0.0);
      words.extend(numbers.into_iter().take(30));
      (words, size)
    },
    || None,
  );
  assert_eq!(read, [0]);
  assert_eq!(found.decided, OrientationSource::Upright);
}
