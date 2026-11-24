use super::*;
use crate::formula::references;

/// What a formula adds up, each reference as the formula writes it, with a minus before one
/// that is taken away. None when the formula's value is not a sum of cells.
fn added(formula: &str) -> Option<Vec<String>> {
  let found = references(formula);
  let adds = shape(formula, &found).adds?;
  Some(
    adds
      .into_iter()
      .map(|(at, sign)| {
        let written = &formula[found[at].start..found[at].end];
        if sign < 0.0 {
          format!("-{written}")
        } else {
          written.to_string()
        }
      })
      .collect(),
  )
}

/// The two references a formula multiplies, as it writes them.
fn multiplied(formula: &str) -> Option<(String, String)> {
  let found = references(formula);
  let (first, second) = shape(formula, &found).product?;
  let written = |at: usize| formula[found[at].start..found[at].end].to_string();
  Some((written(first), written(second)))
}

fn these(references: &[&str]) -> Option<Vec<String>> {
  Some(references.iter().map(|text| text.to_string()).collect())
}

#[test]
fn sums_pluses_and_minuses_add_cells_up_and_each_reference_keeps_its_sign() {
  assert_eq!(added("SUM(A1:A3)"), these(&["A1:A3"]));
  assert_eq!(
    added("SUM(A1:A3,C1,D1:D2)"),
    these(&["A1:A3", "C1", "D1:D2"])
  );
  assert_eq!(added("A1+B2-C3"), these(&["A1", "B2", "-C3"]));
  assert_eq!(added("+A1+B2"), these(&["A1", "B2"]));
  assert_eq!(added("SUM(A1:A3)+SUM(B1:B3)"), these(&["A1:A3", "B1:B3"]));
  assert_eq!(added("SUM(Detail!A1:A3)"), these(&["Detail!A1:A3"]));
  assert_eq!(
    added("'Unit prices'!B2+C3"),
    these(&["'Unit prices'!B2", "C3"])
  );
  // A minus before a sum, or before brackets, takes away everything in it.
  assert_eq!(added("A5-SUM(B1:B3)"), these(&["A5", "-B1:B3"]));
  assert_eq!(added("-(A1+A2)"), these(&["-A1", "-A2"]));
  assert_eq!(added("A1-(B1-C1)"), these(&["A1", "-B1", "C1"]));
  assert_eq!(added("SUM(A1:A3,-B1)"), these(&["A1:A3", "-B1"]));
  // A cell alone is the sum of itself.
  assert_eq!(added("D20"), these(&["D20"]));
  assert_eq!(added("(D20)"), these(&["D20"]));
}

#[test]
fn a_sum_is_still_a_sum_when_it_is_rounded_counted_as_a_subtotal_or_written_as_text() {
  assert_eq!(added("ROUND(SUM(A1:A3),2)"), these(&["A1:A3"]));
  assert_eq!(added("ROUND(A1+A2,0)"), these(&["A1", "A2"]));
  assert_eq!(added("SUBTOTAL(9,A1:A3)"), these(&["A1:A3"]));
  assert_eq!(
    added("SUBTOTAL(109,A1:A3,C1:C3)"),
    these(&["A1:A3", "C1:C3"])
  );
  assert_eq!(added("TEXT(SUM(A1:A3),\"#,##0.00\")"), these(&["A1:A3"]));
  assert_eq!(added("VALUE(A1)+N(B1)"), these(&["A1", "B1"]));
  // Joined to words, or to a cell that holds them, it is the same sum.
  assert_eq!(
    added("\"Total: \"&TEXT(SUM(A1:A3),\"0.00\")"),
    these(&["A1:A3"])
  );
  assert_eq!(
    added("K1&\" \"&TEXT(SUM(A1:A3),\"0.00\")"),
    these(&["A1:A3"])
  );
  // The other kinds of subtotal are not sums: an average, a count.
  assert_eq!(added("SUBTOTAL(1,A1:A3)"), None);
  assert_eq!(added("SUBTOTAL(2,A1:A3)"), None);
}

#[test]
fn a_sum_kept_blank_until_there_is_something_to_show_is_the_sum_it_shows() {
  // Only what is shown is added up: what the question asks about is not.
  assert_eq!(
    added("IF(SUM(D4:D12)>0,SUM(D4:D12),\"\")"),
    these(&["D4:D12"])
  );
  assert_eq!(added("IF(B5>0,SUM(G1:G3),0)"), these(&["G1:G3"]));
  assert_eq!(added("IF(A1=\"\",\"\",SUM(B1:B3))"), these(&["B1:B3"]));
  assert_eq!(added("IF(A1=\"\",,SUM(B1:B3))"), these(&["B1:B3"]));
  assert_eq!(added("IF(A1>0,SUM(B1:B3))"), these(&["B1:B3"]));
  assert_eq!(added("IFERROR(SUM(A1:A3),0)"), these(&["A1:A3"]));
  assert_eq!(added("IFERROR(SUM(A1:A3),\"\")"), these(&["A1:A3"]));
  assert_eq!(
    added("IF(SUM(G1:G9)>0,ROUND(SUM(G1:G9,H1),2),\"\")"),
    these(&["G1:G9", "H1"])
  );
  // Both answers are the same sum, written two ways.
  assert_eq!(
    added("IF(K1=\"\",TEXT(SUM(E5:E9),\"0.00\"),K1&\" \"&TEXT(SUM(E5:E9),\"0.00\"))"),
    these(&["E5:E9"])
  );
  assert_eq!(
    added(
      "IF(K1=\"\",TEXT(SUM(A1:A3)+SUM(B1:B3),\"0.00\"),K1&\" \"&TEXT(SUM(A1:A3)+SUM(B1:B3),\"0.00\"))"
    ),
    these(&["A1:A3", "B1:B3"])
  );
  // Two answers that are different sums: which one it is depends on what cannot be seen.
  assert_eq!(added("IF(A1>0,SUM(B1:B3),SUM(C1:C3))"), None);
  assert_eq!(added("IF(A1>0,SUM(B1:B3),B1-B2)"), None);
  assert_eq!(added("IFERROR(SUM(A1:A3),B1)"), None);
}

#[test]
fn what_does_more_than_add_cells_up_is_not_a_sum() {
  for formula in [
    "A3+5",
    "SUM(A1:A3)*1.1",
    "SUM(A1:A3)/2",
    "A1*B1",
    "A1%",
    "A1^2",
    "A1&B1",
    "A1>B1",
    "MAX(A1:A3)",
    "SUM(A1:A3*B1:B3)",
    "SUMPRODUCT(A1:A3,B1:B3)",
    "VLOOKUP(A1,C1:D9,2,FALSE)",
    "ROUND(SUM(A1:A3),B9)",
    "SUM(A1:A3)+Rate",
    "SUM([1]Costs!A1:A3)",
    "SUM(A1:A3)+#REF!",
    "\"words\"",
    "42",
    "",
    "SUM(A1:A3",
    "SUM(A1:A3))",
    "A1+",
    "SUM()",
  ] {
    assert_eq!(added(formula), None, "{formula:?}");
  }
}

#[test]
fn one_cell_times_another_is_seen_through_what_only_guards_or_rounds_it() {
  let both = |first: &str, second: &str| Some((first.to_string(), second.to_string()));
  assert_eq!(multiplied("B2*C2"), both("B2", "C2"));
  assert_eq!(multiplied("(B2*C2)"), both("B2", "C2"));
  assert_eq!(multiplied("B7*$B$2"), both("B7", "$B$2"));
  assert_eq!(multiplied("Rates!B2*C2"), both("Rates!B2", "C2"));
  assert_eq!(multiplied("ROUND(B2*C2,2)"), both("B2", "C2"));
  assert_eq!(multiplied("IF(B2=\"\",\"\",B2*C2)"), both("B2", "C2"));
  assert_eq!(
    multiplied("IF(OR(B2=\"\",C2=\"\"),\"\",B2*C2)"),
    both("B2", "C2")
  );
  assert_eq!(multiplied("IF(B2>0,B2*C2,0)"), both("B2", "C2"));
  assert_eq!(multiplied("IFERROR(B2*C2,0)"), both("B2", "C2"));
  assert_eq!(multiplied("TEXT(B2*C2,\"0.00\")"), both("B2", "C2"));
  for formula in [
    "B2*C2*D2",
    "B2*2",
    "B2*C2+D2",
    "B2/C2",
    "SUM(B2:B4)*C2",
    "B2:B4*C2:C4",
    "IF(A1=\"x\",B2,C2)*D2",
    "IF(A1>0,B2*C2,B2*D2)",
    "B2+C2",
    "PRODUCT(B2,C2)",
  ] {
    assert_eq!(multiplied(formula), None, "{formula:?}");
  }
}

#[test]
fn a_formula_of_any_length_or_depth_is_read_without_running_out_of_room() {
  // Brackets a thousand deep, and a thousand cells added one by one.
  let deep = format!("{}A1{}", "(".repeat(1000), ")".repeat(1000));
  assert_eq!(added(&deep), None);
  let shallow = format!("{}A1{}", "(".repeat(20), ")".repeat(20));
  assert_eq!(added(&shallow), these(&["A1"]));
  let long: Vec<String> = (1..=1000).map(|row| format!("A{row}")).collect();
  assert_eq!(added(&long.join("+")).map(|all| all.len()), Some(1000));
}
