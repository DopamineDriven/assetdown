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
