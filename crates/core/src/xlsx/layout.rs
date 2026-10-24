//! Worksheet layout recognition for Source Data style sheets: several panels
//! side by side, multi-row headers, placeholder cells and stacked segments
//! separated by repeated header rows. Each recognized table is rewritten as a
//! single header row followed by aligned data rows.

use plotium_io::xlsx::{XlsxCell, XlsxSheet, XlsxValue};

/// Cells that mark a missing value in a numeric column.
const PLACEHOLDERS: [&str; 11] = [
    "--", "-", "—", "–", "NaN", "nan", "N/A", "n/a", "NA", "n.d.", "N.D.",
];

pub(super) struct LaidOutTable {
    pub name: String,
    pub range: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<XlsxCell>>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Empty,
    Placeholder,
    Value,
    Text,
}

fn kind(cell: Option<&XlsxCell>) -> Kind {
    match cell.map(|cell| &cell.value) {
        None | Some(XlsxValue::Empty) => Kind::Empty,
        Some(XlsxValue::Error(_)) => Kind::Placeholder,
        Some(XlsxValue::Utf8(text)) => {
            let text = text.trim();
            if text.is_empty() {
                Kind::Empty
            } else if PLACEHOLDERS.contains(&text) {
                Kind::Placeholder
            } else if super::cell_type(&XlsxValue::Utf8(text.to_owned()))
                .is_some_and(|kind| kind != plotium_data::LogicalType::Utf8)
            {
                Kind::Value
            } else {
                Kind::Text
            }
        }
        Some(_) => Kind::Value,
    }
}

fn header_text(cell: Option<&XlsxCell>) -> String {
    match cell.map(|cell| &cell.value) {
        Some(XlsxValue::Utf8(text)) if text.trim() != "--" => text.trim().to_owned(),
        Some(XlsxValue::Int64(value)) => value.to_string(),
        Some(XlsxValue::Float64(value)) => value.to_string(),
        Some(XlsxValue::Boolean(value)) => value.to_string(),
        Some(
            XlsxValue::DateTimeIso(text)
            | XlsxValue::DurationIso(text)
            | XlsxValue::ExcelDateTime { iso8601: text, .. },
        ) => text.clone(),
        _ => String::new(),
    }
}

/// Absolute addressing over a sheet whose rows start at its origin.
pub(super) struct Grid<'a> {
    sheet: &'a XlsxSheet,
    pub rows: u32,
    pub columns: u32,
}

impl<'a> Grid<'a> {
    pub(super) fn new(sheet: &'a XlsxSheet) -> Self {
        let width = sheet.rows.iter().map(Vec::len).max().unwrap_or(0) as u32;
        Self {
            sheet,
            rows: sheet.origin.0 + sheet.rows.len() as u32,
            columns: sheet.origin.1 + width,
        }
    }

    fn get(&self, row: u32, column: u32) -> Option<&'a XlsxCell> {
        let row = row.checked_sub(self.sheet.origin.0)?;
        let column = column.checked_sub(self.sheet.origin.1)?;
        self.sheet.rows.get(row as usize)?.get(column as usize)
    }

    fn kind(&self, row: u32, column: u32) -> Kind {
        kind(self.get(row, column))
    }

    pub(super) fn row_cells(&self, row: u32, columns: &[u32]) -> Vec<u32> {
        columns
            .iter()
            .copied()
            .filter(|&column| self.kind(row, column) != Kind::Empty)
            .collect()
    }
}

/// Whether the sheet-level reader reads the sheet as it is: one row of distinct
/// text headers over one block whose columns each hold only values or only
/// text, so recognition has nothing to change.
fn is_plain(sheet: &XlsxSheet) -> bool {
    let Some((header, data)) = sheet.rows.split_first() else {
        return false;
    };
    let width = sheet.rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut names = std::collections::HashSet::new();
    let header_ok = header.len() == width
        && header.iter().all(|cell| {
            matches!(&cell.value, XlsxValue::Utf8(text) if !text.trim().is_empty())
                && names.insert(header_text(Some(cell)))
        });
    header_ok
        && (0..width).all(|column| {
            let kinds = data
                .iter()
                .map(|row| kind(row.get(column)))
                .filter(|kind| *kind != Kind::Empty)
                .collect::<Vec<_>>();
            !kinds.is_empty()
                && (kinds.iter().all(|kind| *kind == Kind::Value)
                    || kinds.iter().all(|kind| *kind != Kind::Value))
        })
}

/// Recognizes every table of a sheet. Returns nothing for plain sheets and
/// sheets without tables, leaving them to the sheet-level reader.
pub(super) fn auto_tables(sheet: &XlsxSheet) -> Vec<LaidOutTable> {
    if is_plain(sheet) {
        return Vec::new();
    }
    let grid = Grid::new(sheet);
    let all = (sheet.origin.1..grid.columns).collect::<Vec<_>>();
    let first_row = (sheet.origin.0..grid.rows).find(|&row| !grid.row_cells(row, &all).is_empty());
    let Some(first_row) = first_row else {
        return Vec::new();
    };
    let letters = panel_letters(&grid, first_row, &all);
    let body_start = if letters.is_empty() {
        first_row
    } else {
        first_row + 1
    };
    let occupied =
        |column: u32| (body_start..grid.rows).any(|row| grid.kind(row, column) != Kind::Empty);
    let mut blocks: Vec<Vec<u32>> = Vec::new();
    for column in all {
        let starts_panel = letters
            .iter()
            .any(|(letter_column, _)| *letter_column == column);
        if (!occupied(column) || starts_panel)
            && blocks.last().is_some_and(|block| !block.is_empty())
        {
            blocks.push(Vec::new());
        }
        if occupied(column) {
            match blocks.last_mut() {
                Some(block) => block.push(column),
                None => blocks.push(vec![column]),
            }
        }
    }
    blocks.retain(|block| !block.is_empty());
    let single = blocks.len() == 1 && letters.is_empty();
    let mut tables = blocks
        .iter()
        .filter_map(|columns| {
            let letter = letters
                .iter()
                .find(|(column, _)| *column == columns[0])
                .map(|(_, letter)| letter.clone());
            let mut table = lay_out(&grid, columns, body_start, grid.rows, None, true)?;
            table.name = match (letter, table.title.first()) {
                (Some(letter), _) => format!("{} {letter}", sheet.name),
                (None, _) if single => sheet.name.clone(),
                (None, Some(title)) => format!("{} {title}", sheet.name),
                (None, None) => format!("{} {}", sheet.name, table.range),
            };
            Some(table.finish(sheet, !single))
        })
        .collect::<Vec<_>>();
    let names = tables
        .iter()
        .map(|table| table.name.clone())
        .collect::<Vec<_>>();
    for (table, name) in tables.iter_mut().zip(unique(names)) {
        table.name = name;
    }
    tables
}

/// A first row of single letters over separate columns names side-by-side
/// panels. Adjacent letters are ordinary one-letter headers such as x and y.
fn panel_letters(grid: &Grid<'_>, row: u32, columns: &[u32]) -> Vec<(u32, String)> {
    let cells = grid.row_cells(row, columns);
    let letters = cells
        .iter()
        .filter_map(|&column| {
            let text = header_text(grid.get(row, column));
            let letter = text.trim_matches(|c| c == '(' || c == ')');
            (letter.len() == 1 && letter.chars().all(char::is_alphabetic))
                .then(|| (column, letter.to_owned()))
        })
        .collect::<Vec<_>>();
    let separated = letters.windows(2).all(|pair| pair[1].0 > pair[0].0 + 1);
    // Panels carry their own header rows; a header such as `t` stands
    // directly over values.
    let next = (row + 1..grid.rows).find(|&row| !grid.row_cells(row, columns).is_empty());
    let headed = next.is_some_and(|next| {
        columns
            .iter()
            .all(|&column| grid.kind(next, column) != Kind::Value)
    });
    if !letters.is_empty() && letters.len() == cells.len() && separated && headed {
        letters
    } else {
        Vec::new()
    }
}

pub(super) struct Draft {
    pub name: String,
    pub range: String,
    title: Vec<String>,
    headers: Vec<String>,
    rows: Vec<Vec<XlsxCell>>,
    header_rows: usize,
    notes: Vec<String>,
}

impl Draft {
    pub(super) fn finish(self, sheet: &XlsxSheet, located: bool) -> LaidOutTable {
        let mut diagnostics = Vec::new();
        if located || self.header_rows > 1 {
            diagnostics.push(format!(
                "Read {} of worksheet '{}' with {} header row(s).",
                self.range, sheet.name, self.header_rows
            ));
        }
        if !self.title.is_empty() {
            diagnostics.push(format!("Title: {}.", self.title.join("; ")));
        }
        diagnostics.extend(self.notes);
        LaidOutTable {
            name: self.name,
            range: self.range,
            headers: self.headers,
            rows: self.rows,
            diagnostics,
        }
    }
}

struct Column {
    column: u32,
    name: String,
    /// Absolute row spans of the column's values, one per stacked segment.
    segments: Vec<(u32, u32)>,
    numeric: bool,
}

pub(super) fn lay_out(
    grid: &Grid<'_>,
    columns: &[u32],
    start: u32,
    end: u32,
    header_rows: Option<usize>,
    split: bool,
) -> Option<Draft> {
    let rows = (start..end)
        .filter(|&row| !grid.row_cells(row, columns).is_empty())
        .collect::<Vec<_>>();
    let value_row = rows
        .iter()
        .copied()
        .find(|&row| columns.iter().any(|&c| grid.kind(row, c) == Kind::Value));
    let first = match (header_rows, value_row) {
        (Some(count), _) => start.checked_add(u32::try_from(count).ok()?)?,
        (None, Some(row)) => row,
        // A block of text alone, such as sample notes, keeps its top row as
        // the header.
        (None, None) => *rows.get(1)?,
    };
    let last = *rows.last()?;
    if first > last {
        return None;
    }
    let header = rows
        .iter()
        .copied()
        .filter(|&row| row < first)
        .collect::<Vec<_>>();
    let mut notes = Vec::new();
    let mut placeholders = 0;
    let mut dropped = Vec::new();
    let mut kept = Vec::new();
    for &column in columns {
        let kinds = (first..=last)
            .map(|row| grid.kind(row, column))
            .collect::<Vec<_>>();
        let values = kinds.iter().filter(|kind| **kind == Kind::Value).count();
        let texts = kinds.iter().filter(|kind| **kind == Kind::Text).count();
        if values == 0 && texts == 0 {
            let label = header
                .iter()
                .map(|&row| header_text(grid.get(row, column)))
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>();
            if !label.is_empty() {
                dropped.push(format!(
                    "{} ('{}')",
                    column_letters(column),
                    label.join(" ")
                ));
            }
            continue;
        }
        kept.push((column, values > 0 && values >= texts));
    }
    if !dropped.is_empty() {
        notes.push(format!(
            "Dropped columns with a header but no data: {}.",
            dropped.join(", ")
        ));
    }
    let data_columns = kept.iter().map(|(column, _)| *column).collect::<Vec<_>>();
    // In a table with a text column, a full row of text headers ends the
    // header: a first record such as a blank sample without a value is data.
    let top = rows[0];
    if header_rows.is_none()
        && header.len() > 1
        && kept.iter().any(|(_, numeric)| !numeric)
        && data_columns
            .iter()
            .all(|&column| grid.kind(top, column) == Kind::Text)
    {
        return lay_out(grid, columns, top, end, Some(1), split);
    }
    let title = header
        .iter()
        .copied()
        .filter(|&row| {
            data_columns.len() > 1
                && grid.row_cells(row, &data_columns).len() <= 1
                && grid.row_cells(row, columns).len() == 1
        })
        .collect::<Vec<_>>();
    let names_rows = header
        .iter()
        .copied()
        .filter(|row| !title.contains(row))
        .collect::<Vec<_>>();
    let full = names_rows
        .iter()
        .copied()
        .filter(|&row| grid.row_cells(row, &data_columns).len() == data_columns.len())
        .collect::<Vec<_>>();
    let period = full.last().and_then(|&row| {
        period(
            &data_columns
                .iter()
                .map(|&c| header_text(grid.get(row, c)))
                .collect::<Vec<_>>(),
        )
    });
    let group_of = |index: usize| period.map_or(0, |period| index / period);
    let scans = kept
        .iter()
        .map(|&(column, numeric)| numeric.then(|| scan(grid, column, first, last)))
        .collect::<Vec<_>>();
    let header_texts = names_rows
        .iter()
        .flat_map(|&row| columns.iter().map(move |&c| header_text(grid.get(row, c))))
        .filter(|text| !text.is_empty())
        .collect::<std::collections::HashSet<_>>();
    // A text run splits a column into stacked segments only when every other
    // numeric column of its group that runs across those rows pauses there
    // too, as cyclers write a repeated header between charge and discharge
    // (x columns pause on placeholders). A lone column splits only at a
    // repeated header. Any other text inside values is a missing value.
    let splits = |index: usize, gap: &Gap| {
        let group = (0..scans.len())
            .filter(|&other| group_of(other) == group_of(index))
            .filter_map(|other| scans[other].as_ref().map(|scan| (other, scan)))
            .collect::<Vec<_>>();
        let overlaps = |other: &Gap| other.start <= gap.end && gap.start <= other.end;
        let texts = group
            .iter()
            .flat_map(|(_, scan)| &scan.gaps)
            .filter(|other| overlaps(other))
            .flat_map(|other| &other.texts)
            .collect::<Vec<_>>();
        let paused = group.iter().all(|(other, scan)| {
            *other == index
                || !(scan.span.0 < gap.start && gap.end < scan.span.1)
                || scan.gaps.iter().any(overlaps)
        });
        split
            && paused
            && !texts.is_empty()
            && (group.len() > 1 || texts.iter().any(|text| header_texts.contains(*text)))
    };
    let decisions = scans
        .iter()
        .enumerate()
        .map(|(index, scan)| {
            scan.as_ref().map_or_else(Vec::new, |scan| {
                scan.gaps.iter().map(|gap| splits(index, gap)).collect()
            })
        })
        .collect::<Vec<Vec<bool>>>();
    let mut built = Vec::new();
    let mut breaks = Vec::new();
    let mut trailing = Vec::new();
    let mut missing_text = Vec::new();
    for (index, ((&(column, numeric), scan), decisions)) in
        kept.iter().zip(scans).zip(decisions).enumerate()
    {
        let mut parts = names_rows
            .iter()
            .filter_map(|&row| {
                let text = match period {
                    Some(period) if !full.contains(&row) => {
                        let group = index / period * period;
                        data_columns[group..group + period]
                            .iter()
                            .map(|&c| header_text(grid.get(row, c)))
                            .find(|text| !text.is_empty())
                            .unwrap_or_default()
                    }
                    _ => header_text(grid.get(row, column)),
                };
                (!text.is_empty()).then_some(text)
            })
            .collect::<Vec<_>>();
        let segments = match scan {
            Some(scan) => {
                placeholders += scan.placeholders;
                parts.extend(scan.leading);
                trailing.extend(scan.trailing);
                let mut segments = Vec::new();
                let mut start = scan.span.0;
                for (gap, splits) in scan.gaps.into_iter().zip(decisions) {
                    if splits {
                        segments.push((start, gap.start - 1));
                        start = gap.end + 1;
                        breaks.push(start);
                    } else {
                        missing_text.extend(gap.texts);
                    }
                }
                segments.push((start, scan.span.1));
                segments
            }
            None => {
                let mut filled =
                    (first..=last).filter(|&row| grid.kind(row, column) != Kind::Empty);
                let top = filled.next().unwrap_or(first);
                vec![(top, filled.next_back().unwrap_or(top))]
            }
        };
        built.push(Column {
            column,
            name: if parts.is_empty() {
                format!("Column {}", column_letters(column))
            } else {
                parts.join(" ")
            },
            segments,
            numeric,
        });
    }
    if placeholders > 0 {
        notes.push(format!(
            "Read {placeholders} placeholder cell(s) such as '--' as missing values."
        ));
    }
    if !breaks.is_empty() {
        breaks.sort_unstable();
        breaks.dedup();
        let rows = breaks
            .iter()
            .map(|row| (row + 1).to_string())
            .collect::<Vec<_>>();
        notes.push(format!(
            "Split stacked segments separated by repeated header rows into separate columns (segments start at row {}).",
            rows.join(", ")
        ));
    }
    if !missing_text.is_empty() {
        notes.push(format!(
            "Read {} text cell(s) inside numeric columns as missing values: {}.",
            missing_text.len(),
            quoted(&missing_text)
        ));
    }
    if !trailing.is_empty() {
        notes.push(format!(
            "Ignored {} text cell(s) below the data in numeric columns: {}.",
            trailing.len(),
            quoted(&trailing)
        ));
    }
    if built
        .iter()
        .all(|column| column.name.starts_with("Column "))
    {
        notes.push("No header row was found; named columns by their letters.".into());
    }
    let (headers, rows) = assemble(grid, &built, period, first);
    Some(Draft {
        name: String::new(),
        range: format!(
            "{}{}:{}{}",
            column_letters(columns[0]),
            header.first().unwrap_or(&first) + 1,
            column_letters(*columns.last()?),
            last + 1
        ),
        title: title
            .iter()
            .map(|&row| {
                grid.row_cells(row, columns)
                    .first()
                    .map(|&c| header_text(grid.get(row, c)))
                    .unwrap_or_default()
            })
            .collect(),
        headers: unique(headers),
        rows,
        header_rows: header.len(),
        notes,
    })
}

/// A run of rows without values between two values of a column.
struct Gap {
    start: u32,
    end: u32,
    texts: Vec<String>,
}

struct Scan {
    /// First and last value rows.
    span: (u32, u32),
    gaps: Vec<Gap>,
    /// Text above the first value, read as more header.
    leading: Vec<String>,
    /// Text below the last value, such as notes.
    trailing: Vec<String>,
    placeholders: usize,
}

fn scan(grid: &Grid<'_>, column: u32, first: u32, last: u32) -> Scan {
    let mut result = Scan {
        span: (first, first),
        gaps: Vec::new(),
        leading: Vec::new(),
        trailing: Vec::new(),
        placeholders: 0,
    };
    let mut seen_value = false;
    let mut gap: Option<Gap> = None;
    for row in first..=last {
        match grid.kind(row, column) {
            Kind::Value => {
                if !seen_value {
                    result.span.0 = row;
                    seen_value = true;
                } else if let Some(mut gap) = gap.take() {
                    gap.end = row - 1;
                    result.gaps.push(gap);
                }
                result.trailing.clear();
                result.span.1 = row;
            }
            kind if seen_value => {
                let entry = gap.get_or_insert(Gap {
                    start: row,
                    end: row,
                    texts: Vec::new(),
                });
                if kind == Kind::Text {
                    let text = header_text(grid.get(row, column));
                    entry.texts.push(text.clone());
                    result.trailing.push(text);
                }
                result.placeholders += usize::from(kind == Kind::Placeholder);
            }
            Kind::Text => result.leading.push(header_text(grid.get(row, column))),
            _ => {}
        }
    }
    result
}

fn quoted(texts: &[String]) -> String {
    let shown = texts
        .iter()
        .take(4)
        .map(|text| format!("'{text}'"))
        .collect::<Vec<_>>()
        .join(", ");
    if texts.len() > 4 {
        format!("{shown}, …")
    } else {
        shown
    }
}

/// Orders output columns segment by segment within each header period, so
/// x/y pairs stay adjacent, and aligns each segment at its earliest start.
fn assemble(
    grid: &Grid<'_>,
    columns: &[Column],
    period: Option<usize>,
    first: u32,
) -> (Vec<String>, Vec<Vec<XlsxCell>>) {
    let group = period.unwrap_or(columns.len()).max(1);
    let segment_count =
        |columns: &[Column]| columns.iter().map(|c| c.segments.len()).max().unwrap_or(0);
    let starts = (0..segment_count(columns))
        .map(|segment| {
            columns
                .iter()
                .filter_map(|c| c.segments.get(segment).map(|span| span.0))
                .min()
                .unwrap_or(first)
        })
        .collect::<Vec<_>>();
    let mut order = Vec::new();
    for chunk in columns.chunks(group) {
        for segment in 0..segment_count(chunk) {
            for column in chunk.iter().filter(|c| segment < c.segments.len()) {
                order.push((column, segment));
            }
        }
    }
    let headers = order
        .iter()
        .map(|(column, segment)| {
            if column.segments.len() > 1 {
                format!("{} ({})", column.name, segment + 1)
            } else {
                column.name.clone()
            }
        })
        .collect();
    let height = order
        .iter()
        .map(|(column, segment)| column.segments[*segment].1 + 1 - starts[*segment])
        .max()
        .unwrap_or(0);
    let rows = (0..height)
        .map(|offset| {
            order
                .iter()
                .map(|(column, segment)| {
                    let (from, to) = column.segments[*segment];
                    let row = starts[*segment] + offset;
                    let cell = grid
                        .get(row, column.column)
                        .filter(|_| (from..=to).contains(&row));
                    match (cell, kind(cell)) {
                        (Some(cell), Kind::Value) => cell.clone(),
                        (Some(cell), Kind::Text) if !column.numeric => cell.clone(),
                        (Some(cell), Kind::Placeholder) if !column.numeric => cell.clone(),
                        _ => XlsxCell::value(XlsxValue::Empty),
                    }
                })
                .collect()
        })
        .collect();
    (headers, rows)
}

/// The shortest repeat of the bottom header row, such as `2 theta, intensity`
/// repeated for each sample.
fn period(names: &[String]) -> Option<usize> {
    let count = names.len();
    (1..count)
        .filter(|period| count.is_multiple_of(*period))
        .find(|&period| (period..count).all(|index| names[index] == names[index % period]))
}

fn unique(names: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    names
        .into_iter()
        .map(|name| {
            let mut candidate = name.clone();
            let mut index = 2;
            while !seen.insert(candidate.clone()) {
                candidate = format!("{name} ({index})");
                index += 1;
            }
            candidate
        })
        .collect()
}

pub(super) fn column_letters(column: u32) -> String {
    let mut column = column + 1;
    let mut letters = Vec::new();
    while column > 0 {
        let rem = (column - 1) % 26;
        letters.push(b'A' + rem as u8);
        column = (column - 1) / 26;
    }
    letters.reverse();
    String::from_utf8(letters).expect("ASCII letters")
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
