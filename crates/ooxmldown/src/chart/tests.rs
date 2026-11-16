use super::*;

const C: &str = r#"xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#;

fn chart(inner: &str) -> Chart {
  read_chart(format!("<c:chartSpace {C}><c:chart>{inner}</c:chart></c:chartSpace>").as_bytes())
    .unwrap()
}

fn strings(values: &[&str]) -> String {
  let points: String = values
    .iter()
    .enumerate()
    .map(|(at, value)| format!(r#"<c:pt idx="{at}"><c:v>{value}</c:v></c:pt>"#))
    .collect();
  format!(
    r#"<c:strRef><c:f>Sheet1!$A$2:$A$9</c:f><c:strCache><c:ptCount val="{}"/>{points}</c:strCache></c:strRef>"#,
    values.len()
  )
}

fn numbers(count: usize, points: &[(usize, &str)]) -> String {
  let points: String = points
    .iter()
    .map(|(at, value)| format!(r#"<c:pt idx="{at}"><c:v>{value}</c:v></c:pt>"#))
    .collect();
  format!(
    r#"<c:numRef><c:f>Sheet1!$B$2:$B$9</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="{count}"/>{points}</c:numCache></c:numRef>"#
  )
}

#[test]
fn a_chart_is_its_title_and_the_cached_points_of_each_series() {
  let found = chart(&format!(
    r#"<c:title><c:tx><c:rich><a:p><a:r><a:t>Revenue </a:t></a:r><a:r><a:t>by region</a:t></a:r></a:p></c:rich></c:tx></c:title>
       <c:plotArea><c:barChart><c:barDir val="col"/>
         <c:ser><c:tx>{}</c:tx><c:cat>{}</c:cat><c:val>{}</c:val></c:ser>
         <c:ser><c:tx><c:v>Plan</c:v></c:tx><c:cat>{}</c:cat><c:val>{}</c:val></c:ser>
       </c:barChart></c:plotArea>"#,
    strings(&["Actual"]),
    strings(&["North", "South", "West"]),
    numbers(3, &[(0, "12"), (1, "15.5"), (2, "-3")]),
    strings(&["North", "South", "West"]),
    // A point the cache skips has no value; one that is not a number has none either.
    numbers(3, &[(0, "10"), (2, "n/a")]),
  ));
  assert_eq!(found.title.as_deref(), Some("Revenue by region"));
  assert_eq!(
    found.series,
    [
      ChartSeries {
        kind: ChartKind::Bar,
        name: Some("Actual".to_owned()),
        categories: vec!["North".to_owned(), "South".to_owned(), "West".to_owned()],
        values: vec![Some(12.0), Some(15.5), Some(-3.0)],
      },
      ChartSeries {
        kind: ChartKind::Bar,
        name: Some("Plan".to_owned()),
        categories: vec!["North".to_owned(), "South".to_owned(), "West".to_owned()],
        values: vec![Some(10.0), None, None],
      },
    ]
  );
}

#[test]
fn a_chart_that_mixes_kinds_has_series_of_each_and_scatter_points_have_x_values() {
  let found = chart(&format!(
    r#"<c:plotArea><c:layout/>
         <c:bar3DChart><c:ser><c:val>{}</c:val></c:ser></c:bar3DChart>
         <c:lineChart><c:ser><c:val>{}</c:val></c:ser></c:lineChart>
         <c:scatterChart><c:ser><c:xVal>{}</c:xVal><c:yVal>{}</c:yVal></c:ser></c:scatterChart>
         <c:ofPieChart><c:ser/></c:ofPieChart>
         <c:catAx/><c:valAx/>
       </c:plotArea>"#,
    numbers(1, &[(0, "1")]),
    numbers(1, &[(0, "2")]),
    numbers(2, &[(0, "0.5"), (1, "1.5")]),
    numbers(2, &[(0, "7"), (1, "9")]),
  ));
  let kinds: Vec<ChartKind> = found.series.iter().map(|s| s.kind).collect();
  assert_eq!(
    kinds,
    [
      ChartKind::Bar,
      ChartKind::Line,
      ChartKind::Scatter,
      ChartKind::Pie
    ]
  );
  assert_eq!(found.title, None);
  assert_eq!(found.series[2].categories, ["0.5", "1.5"]);
  assert_eq!(found.series[2].values, [Some(7.0), Some(9.0)]);
  assert!(found.series[3].values.is_empty() && found.series[3].name.is_none());
}

#[test]
fn a_cache_that_claims_more_points_than_a_series_may_hold_is_cut_short() {
  let found = chart(&format!(
    "<c:plotArea><c:lineChart><c:ser><c:val>{}</c:val></c:ser></c:lineChart></c:plotArea>",
    numbers(4_000_000_000, &[(0, "1"), (3_999_999_999, "2")]),
  ));
  assert_eq!(found.series[0].values.len(), MAX_POINTS);
  assert_eq!(found.series[0].values[0], Some(1.0));
}

#[test]
fn a_part_with_no_chart_in_it_is_a_chart_with_no_series_and_bad_xml_is_an_error() {
  let empty = read_chart(format!("<c:chartSpace {C}/>").as_bytes()).unwrap();
  assert_eq!((empty.title, empty.series.len()), (None, 0));
  assert!(read_chart(b"<c:chartSpace").is_err());
}
