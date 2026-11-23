//! A workbook for the tests, made as `xlsdown` makes one: a sheet of cells in `xlsxdown`'s model.

use std::collections::BTreeMap;

use xlsxdown::{
  Cell, CellRange, DocProps, Horizontal, Sheet, Stored, Styles, Vertical, Workbook, Xf,
};

fn xf(num_fmt_id: u32) -> Xf {
  Xf {
    num_fmt_id,
    font_id: 0,
    horizontal: Horizontal::General,
    vertical: Vertical::Bottom,
    wrap: false,
    indent: 0,
  }
}

fn sheet(name: &str, index: u32, hidden: bool, cells: BTreeMap<(u32, u32), Cell>) -> Sheet {
  Sheet {
    name: name.to_string(),
    index,
    hidden,
    dimension: CellRange::parse("A1:B3"),
    print_area: None,
    cols: Vec::new(),
    rows: BTreeMap::new(),
    default_row_height: 12.75,
    default_col_width: None,
    cells,
    merges: Vec::new(),
    drawings: 0,
    links: 0,
    tables: 0,
    comments: 0,
    pictures: 0,
    charts: 0,
    pivot_tables: 0,
    warnings: Vec::new(),
  }
}

/// Two sheets, the second hidden. The first holds a label, a date, a number with a formula of
/// its own, and a number that shares that formula; the second holds one label.
pub(crate) fn workbook() -> Workbook {
  let styles = Styles {
    xfs: vec![xf(0), xf(14), xf(4)],
    ..Styles::default()
  };
  let mut warnings = Vec::new();
  let mut styled = |stored: Stored, style: usize, reference: &str| {
    Cell::styled(stored, style, &styles, false, reference, &mut warnings)
  };
  let own = Cell {
    formula: Some("A2*2".to_string()),
    cached: true,
    ..styled(Stored::number(1234.5), 2, "B2")
  };
  let follower = Cell {
    formula: Some("A3*2".to_string()),
    shared_master: Some((1, 1)),
    cached: true,
    ..styled(Stored::number(8.0), 0, "B3")
  };
  let first = BTreeMap::from([
    ((0, 0), styled(Stored::text("Total".to_string()), 0, "A1")),
    ((0, 1), styled(Stored::number(46349.0), 1, "B1")),
    ((1, 1), own),
    ((2, 1), follower),
  ]);
  let second = BTreeMap::from([(
    (0, 0),
    styled(Stored::text("kept back".to_string()), 0, "A1"),
  )]);
  Workbook {
    sheets: vec![
      sheet("Costs", 0, false, first),
      sheet("Notes", 1, true, second),
    ],
    date1904: false,
    styles,
    props: DocProps::default(),
    defined_names: 0,
    has_macros: false,
    is_template: false,
  }
}
