use super::*;

/// 0, 1, 2 and so on: a byte's value is its own offset, so a read can be checked by sight.
fn counting(length: u8) -> Vec<u8> {
  (0..length).collect()
}

fn piece(offset: u64, bytes: &[u8]) -> Piece<'_> {
  Piece { offset, bytes }
}

fn not_given(offset: u64, length: u64) -> Missing {
  Missing::NotGiven(ByteRange::forward(offset, length))
}

#[test]
fn a_whole_file_is_read_anywhere_inside_it() {
  let file = counting(10);
  let source = Source::whole(&file);
  assert_eq!(source.size(), Some(10));
  assert_eq!(source.head_len(), 10);
  assert_eq!(source.read(0, 4), Ok(&[0, 1, 2, 3][..]));
  assert_eq!(source.read(7, 3), Ok(&[7, 8, 9][..]));
}

#[test]
fn a_whole_file_ends_where_it_ends() {
  let file = counting(10);
  let source = Source::whole(&file);
  // Nothing can be asked for: the file is all there is.
  assert_eq!(source.read(8, 3), Err(Missing::PastEnd));
  assert_eq!(source.read(10, 1), Err(Missing::PastEnd));
  assert_eq!(source.read(u64::MAX, 2), Err(Missing::PastEnd));
}

#[test]
fn pieces_are_read_at_their_own_offsets() {
  let file = counting(100);
  let pieces = [piece(0, &file[..10]), piece(60, &file[60..80])];
  let source = Source::from_pieces(&pieces, Some(100)).unwrap();
  assert_eq!(source.head_len(), 10);
  assert_eq!(source.read(2, 3), Ok(&[2, 3, 4][..]));
  assert_eq!(source.read(60, 2), Ok(&[60, 61][..]));
  assert_eq!(source.read(78, 2), Ok(&[78, 79][..]));
}

#[test]
fn a_read_between_pieces_says_which_bytes_are_missing() {
  let file = counting(100);
  let pieces = [piece(0, &file[..10]), piece(60, &file[60..80])];
  let source = Source::from_pieces(&pieces, Some(100)).unwrap();
  assert_eq!(source.read(20, 5), Err(not_given(20, 5)));
  assert_eq!(source.read(90, 10), Err(not_given(90, 10)));
}

#[test]
fn a_read_that_runs_off_a_piece_asks_only_for_what_lies_past_it() {
  let file = counting(100);
  let pieces = [piece(0, &file[..10])];
  let source = Source::from_pieces(&pieces, Some(100)).unwrap();
  assert_eq!(source.read(6, 10), Err(not_given(10, 6)));
}

#[test]
fn past_the_size_given_there_is_nothing_to_ask_for() {
  let file = counting(100);
  let pieces = [piece(0, &file[..10])];
  let source = Source::from_pieces(&pieces, Some(100)).unwrap();
  assert_eq!(source.read(98, 4), Err(Missing::PastEnd));
}

#[test]
fn without_a_size_whatever_follows_the_pieces_may_still_be_there() {
  let file = counting(10);
  let pieces = [piece(0, &file)];
  let source = Source::from_pieces(&pieces, None).unwrap();
  assert_eq!(source.size(), None);
  assert_eq!(source.read(8, 6), Err(not_given(10, 4)));
  assert_eq!(source.read(500, 2), Err(not_given(500, 2)));
}

#[test]
fn pieces_that_touch_or_overlap_are_read_as_one() {
  let file = counting(100);
  let touching = [piece(0, &file[..10]), piece(10, &file[10..20])];
  let source = Source::from_pieces(&touching, None).unwrap();
  assert_eq!(source.head_len(), 20);
  assert_eq!(source.read(8, 4), Ok(&[8, 9, 10, 11][..]));

  let overlapping = [piece(0, &file[..10]), piece(4, &file[4..30])];
  let source = Source::from_pieces(&overlapping, None).unwrap();
  assert_eq!(source.head_len(), 30);
  assert_eq!(source.read(8, 4), Ok(&[8, 9, 10, 11][..]));

  // A piece that lies wholly inside another adds nothing and takes nothing away.
  let inside = [piece(0, &file[..30]), piece(5, &file[5..9])];
  let source = Source::from_pieces(&inside, None).unwrap();
  assert_eq!(source.head_len(), 30);
  assert_eq!(source.read(26, 4), Ok(&[26, 27, 28, 29][..]));
}

#[test]
fn pieces_are_taken_in_whatever_order_they_come() {
  let file = counting(100);
  let pieces = [
    piece(60, &file[60..80]),
    piece(0, &file[..10]),
    piece(10, &file[10..12]),
  ];
  let source = Source::from_pieces(&pieces, None).unwrap();
  assert_eq!(source.head_len(), 12);
  assert_eq!(source.read(9, 3), Ok(&[9, 10, 11][..]));
  assert_eq!(source.read(61, 1), Ok(&[61][..]));
}

#[test]
fn pieces_that_overlap_and_disagree_are_refused() {
  let file = counting(100);
  let other = [9, 9, 9, 9];
  let pieces = [piece(0, &file[..10]), piece(8, &other)];
  let error = Source::from_pieces(&pieces, None).err();
  assert_eq!(
    error,
    Some(MimeError::InvalidRanges(
      "the pieces at offsets 0 and 8 overlap and differ at offset 8".to_string()
    ))
  );
}

#[test]
fn a_piece_that_lies_past_the_size_given_is_refused() {
  let file = counting(100);
  let pieces = [piece(90, &file[..20])];
  let error = Source::from_pieces(&pieces, Some(100)).err();
  assert_eq!(
    error,
    Some(MimeError::InvalidRanges(
      "the piece at offset 90 ends at 110, past the file's size of 100".to_string()
    ))
  );
}

#[test]
fn a_file_of_which_nothing_was_given_has_everything_missing() {
  let source = Source::from_pieces(&[], None).unwrap();
  assert_eq!(source.head_len(), 0);
  assert_eq!(source.read(0, 8), Err(not_given(0, 8)));

  // An empty piece is no piece.
  let source = Source::from_pieces(&[piece(40, &[])], Some(50)).unwrap();
  assert_eq!(source.read(40, 2), Err(not_given(40, 2)));
}

#[test]
fn a_read_of_nothing_lacks_nothing() {
  let source = Source::from_pieces(&[], None).unwrap();
  assert_eq!(source.read(40, 0), Ok(&[][..]));
  assert_eq!(Source::whole(&[]).read(0, 0), Ok(&[][..]));
}

#[test]
fn a_few_missing_bytes_are_asked_for_a_block_at_a_time() {
  let file = counting(100);
  let pieces = [piece(0, &file[..10])];
  let open_ended = Source::from_pieces(&pieces, None).unwrap();
  assert_eq!(
    open_ended.block(ByteRange::forward(10, 2)),
    ByteRange::forward(10, 4096)
  );
  // More than a block is asked for as it is, and so is a range counted from the end.
  assert_eq!(
    open_ended.block(ByteRange::forward(10, 9000)),
    ByteRange::forward(10, 9000)
  );
  assert_eq!(open_ended.block(ByteRange::last(22)), ByteRange::last(22));
  // No further than the file goes.
  let sized = Source::from_pieces(&pieces, Some(100)).unwrap();
  assert_eq!(
    sized.block(ByteRange::forward(10, 2)),
    ByteRange::forward(10, 90)
  );
}

#[test]
fn a_range_that_carries_on_from_what_was_given_is_asked_for_as_long_again() {
  let file: Vec<u8> = vec![7; 20_000];
  let pieces = [piece(0, &file[..10_000])];
  let source = Source::from_pieces(&pieces, None).unwrap();
  // Straight on from the start that was given: as much again.
  assert_eq!(
    source.block(ByteRange::forward(10_000, 2)),
    ByteRange::forward(10_000, 10_000)
  );
  // Somewhere else in the file: a block.
  assert_eq!(
    source.block(ByteRange::forward(15_000, 2)),
    ByteRange::forward(15_000, 4096)
  );
}
