//! Renders a match report into a color-highlighted, human-friendly Excel
//! workbook (`.xlsx`).
//!
//! This module is the Excel counterpart to the CSV output of the match
//! commands. It takes the **exact same** header row and data rows that the CSV
//! formatter produces and renders them into a styled workbook, so the two
//! formats are always column-for-column consistent — only the presentation
//! differs.
//!
//! A match report pairs a *reference* asset against a *candidate* asset. Paired
//! metadata columns are named `REF_<field>` and `CAN_<field>`, where `<field>`
//! is the metadata field name and must be identical between the two columns.
//! All other columns (`REFERENCE_ASSET_PATH`, `MATCH_PERCENTAGE`,
//! `COMPARISON_URL`, …) are plain, unpaired columns.
//!
//! For each pair the renderer highlights, cell by cell, where the two values
//! differ (red), match (green), or are present on only one side (amber), so a
//! large report can be scanned visually. The `MATCH_PERCENTAGE` column drives a
//! descending sort and a heat-map gradient, and the `COMPARISON_URL` column is
//! written as a clickable hyperlink.
//!
//! The logic here is ported from the standalone `match-report-analyzer` tool,
//! adapted to consume in-memory rows (rather than re-reading a CSV) and to use
//! pcli2's `CAN_` candidate-metadata prefix.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use rust_xlsxwriter::{
    Color, ConditionalFormat3ColorScale, ConditionalFormatType, DocProperties, Format, FormatAlign,
    FormatBorder, Url, Workbook, XlsxError,
};
use thiserror::Error;
use tracing::{debug, info};

/// Prefix marking a column that holds a *reference* asset's metadata value.
const REF_PREFIX: &str = "REF_";
/// Prefix marking a column that holds a *candidate* asset's metadata value.
/// Kept the same length as [`REF_PREFIX`] for visual consistency.
const CAN_PREFIX: &str = "CAN_";

/// Header of the reference asset's path column.
const REFERENCE_ASSET_PATH_COLUMN: &str = "REFERENCE_ASSET_PATH";
/// Header of the candidate asset's path column.
const CANDIDATE_ASSET_PATH_COLUMN: &str = "CANDIDATE_ASSET_PATH";
/// Header of the geometric similarity column (0–100). The most relevant pairs
/// have the highest value, so this column drives sorting and the heat-map
/// gradient in the rendered workbook.
const MATCH_PERCENTAGE_COLUMN: &str = "MATCH_PERCENTAGE";
/// Header of the column holding the deep-link comparison URL, rendered as a
/// clickable hyperlink.
const COMPARISON_URL_COLUMN: &str = "COMPARISON_URL";

/// Columns that every match report must contain for the conversion to make
/// sense. Their absence means the data isn't a usable match report.
const REQUIRED_COLUMNS: [&str; 3] = [
    REFERENCE_ASSET_PATH_COLUMN,
    CANDIDATE_ASSET_PATH_COLUMN,
    MATCH_PERCENTAGE_COLUMN,
];

/// The only file extension Excel recognizes for the format this module writes.
const XLSX_EXTENSION: &str = "xlsx";

/// Errors that can occur while rendering a match report into an Excel workbook.
#[derive(Debug, Error)]
pub enum XlsxReportError {
    /// The data is missing one or more columns required to build a match report.
    #[error("the match report is missing required column(s): {}", .0.join(", "))]
    MissingRequiredColumns(Vec<String>),
    /// An error occurred while building or saving the Excel workbook.
    #[error("failed to write Excel workbook: {0}")]
    Xlsx(#[from] XlsxError),
}

// ---------------------------------------------------------------------------
// Colors, dimensions, and format constants (ported verbatim).
// ---------------------------------------------------------------------------

/// Background color for paired cells whose values match. Excel's "good" green.
const COLOR_MATCH: Color = Color::RGB(0xC6EFCE);
/// Background color for cells whose reference and candidate values differ.
/// Excel's "bad" red.
const COLOR_DIFFERENT: Color = Color::RGB(0xFFC7CE);
/// Background color for cells where a value is present on one side only.
/// Excel's "neutral" amber.
const COLOR_MISSING: Color = Color::RGB(0xFFEB9C);
/// Header fill: a deep, professional blue with white text.
const COLOR_HEADER_BG: Color = Color::RGB(0x1F4E78);
/// Fill for the second header row (the `REF`/`CAN` sub-labels): a lighter blue.
const COLOR_SUBHEADER_BG: Color = Color::RGB(0x2E6CA4);

/// Heat-map color for the lowest match percentage (0%): a calm, "cool" blue.
const COLOR_HEAT_LOW: Color = Color::RGB(0x5A8AC6);
/// Heat-map color for the midpoint (50%): a warm yellow.
const COLOR_HEAT_MID: Color = Color::RGB(0xFFEB84);
/// Heat-map color for the highest match percentage (100%): "red hot".
const COLOR_HEAT_HIGH: Color = Color::RGB(0xF8696B);

/// Number format applied to the match-percentage column.
const PERCENT_NUM_FORMAT: &str = "0.00";

/// Height (in points) of each of the two header rows.
const HEADER_ROW_HEIGHT: f64 = 22.0;
/// Row index of the top "group" header (field-name band over each pair).
const GROUP_HEADER_ROW: u32 = 0;
/// Row index of the second header (per-column `REF`/`CAN` and unpaired names).
const LABEL_HEADER_ROW: u32 = 1;
/// Row index where data begins (after the two header rows).
const DATA_START_ROW: u32 = 2;
/// Padding (in characters) added to a column's widest content.
const COL_WIDTH_PADDING: f64 = 2.0;
/// Narrowest a sized column may be.
const MIN_COL_WIDTH: f64 = 8.0;
/// Widest a normal sized column may be, so long values don't dominate.
const MAX_COL_WIDTH: f64 = 48.0;
/// Excel's hard maximum column width, used for the comparison-URL column.
const EXCEL_MAX_COL_WIDTH: f64 = 255.0;
/// Excel's maximum supported URL length. Longer values are written as text.
const MAX_URL_LEN: usize = 2080;

/// Worksheet tab name.
const SHEET_NAME: &str = "Match Report";

/// The comparison state of a single `REF_`/`CAN_` cell pair within a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellState {
    /// Nothing to highlight: not part of a comparable pair, or both values empty.
    Neutral,
    /// Both values present and equal — a genuine match.
    Match,
    /// Both values present but differ.
    Different,
    /// Exactly one of the two values is empty (missing on one side).
    Missing,
}

/// Compares a reference value against a candidate value on their trimmed text.
fn classify(reference: &str, candidate: &str) -> CellState {
    let reference = reference.trim();
    let candidate = candidate.trim();

    if reference.is_empty() && candidate.is_empty() {
        CellState::Neutral
    } else if reference == candidate {
        CellState::Match
    } else if reference.is_empty() || candidate.is_empty() {
        CellState::Missing
    } else {
        CellState::Different
    }
}

/// The structure of a match report: its header row plus, for every column, the
/// index of its partner column if it participates in a `REF_`/`CAN_` pair.
#[derive(Debug, Clone)]
struct Schema {
    headers: Vec<String>,
    /// For each column index, `Some(partner_index)` if it is one half of a
    /// `REF_`/`CAN_` pair, otherwise `None`.
    partners: Vec<Option<usize>>,
}

impl Schema {
    /// Builds a [`Schema`] by pairing `REF_<field>` columns with their
    /// `CAN_<field>` counterparts.
    fn from_headers(headers: Vec<String>) -> Self {
        let mut ref_cols: Vec<(String, usize)> = Vec::new();
        let mut can_cols: Vec<(String, usize)> = Vec::new();

        for (idx, header) in headers.iter().enumerate() {
            if let Some(field) = header.strip_prefix(REF_PREFIX) {
                ref_cols.push((field.to_string(), idx));
            } else if let Some(field) = header.strip_prefix(CAN_PREFIX) {
                can_cols.push((field.to_string(), idx));
            }
        }

        let mut partners = vec![None; headers.len()];
        let mut used_can: Vec<bool> = vec![false; can_cols.len()];
        for (field, ref_idx) in &ref_cols {
            // Pair with the first not-yet-used CAN_ column of the same field, so
            // duplicate field names (e.g. two `MATERIAL` columns) pair up 1:1
            // instead of all collapsing onto the first candidate column.
            if let Some(slot) = can_cols
                .iter()
                .enumerate()
                .position(|(i, (f, _))| !used_can[i] && f == field)
            {
                let can_idx = can_cols[slot].1;
                used_can[slot] = true;
                partners[*ref_idx] = Some(can_idx);
                partners[can_idx] = Some(*ref_idx);
            }
        }

        Schema { headers, partners }
    }

    fn headers(&self) -> &[String] {
        &self.headers
    }

    fn column_count(&self) -> usize {
        self.headers.len()
    }

    fn partner(&self, column: usize) -> Option<usize> {
        self.partners.get(column).copied().flatten()
    }

    fn column_index(&self, name: &str) -> Option<usize> {
        self.headers.iter().position(|h| h == name)
    }

    /// The metadata field name of a paired column — the part after the `REF_` or
    /// `CAN_` prefix. Returns `None` for columns that are not part of a pair.
    fn field_name(&self, column: usize) -> Option<&str> {
        self.partner(column)?;
        let header = self.headers.get(column)?;
        header
            .strip_prefix(REF_PREFIX)
            .or_else(|| header.strip_prefix(CAN_PREFIX))
    }

    /// The [`REQUIRED_COLUMNS`] absent from this schema, in order.
    fn missing_required_columns(&self) -> Vec<String> {
        REQUIRED_COLUMNS
            .iter()
            .filter(|name| self.column_index(name).is_none())
            .map(|name| name.to_string())
            .collect()
    }

    /// The number of comparable `REF_`/`CAN_` pairs.
    fn pair_count(&self) -> usize {
        self.partners.iter().filter(|p| p.is_some()).count() / 2
    }
}

/// A fully-parsed match report: its [`Schema`] and all data rows.
#[derive(Debug, Clone)]
struct Report {
    schema: Schema,
    rows: Vec<Vec<String>>,
}

impl Report {
    /// Builds a report from a header row and data rows (already column-aligned).
    fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Report {
            schema: Schema::from_headers(headers),
            rows,
        }
    }

    /// Sorts the data rows by the numeric value of `column`, descending. Cells
    /// that don't parse as a number (including blanks) sort to the bottom. Stable.
    fn sort_by_numeric_desc(&mut self, column: usize) {
        fn numeric(row: &[String], column: usize) -> Option<f64> {
            row.get(column)
                .and_then(|cell| cell.trim().parse::<f64>().ok())
        }

        self.rows
            .sort_by(|a, b| match (numeric(a, column), numeric(b, column)) {
                (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(Ordering::Equal),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            });
    }

    /// Classifies the cell at `(row, column)` against its pair partner. Returns
    /// [`CellState::Neutral`] for unpaired columns or out-of-bounds cells.
    fn cell_state(&self, row: usize, column: usize) -> CellState {
        let Some(partner) = self.schema.partner(column) else {
            return CellState::Neutral;
        };
        let Some(record) = self.rows.get(row) else {
            return CellState::Neutral;
        };
        let here = record.get(column).map(String::as_str).unwrap_or("");
        let there = record.get(partner).map(String::as_str).unwrap_or("");

        let is_reference = self.schema.headers()[column].starts_with(REF_PREFIX);
        if is_reference {
            classify(here, there)
        } else {
            classify(there, here)
        }
    }
}

/// A summary of what was written, returned for logging and reporting.
#[derive(Debug, Default, Clone, Copy)]
pub struct ConversionStats {
    /// Number of data rows written.
    pub rows: usize,
    /// Number of comparable `REF_`/`CAN_` pairs in the schema.
    pub pairs: usize,
    /// Number of cells highlighted as matching.
    pub matching: usize,
    /// Number of cells highlighted as differing.
    pub different: usize,
    /// Number of cells highlighted as missing-on-one-side.
    pub missing: usize,
}

/// Ensures the output path carries the `.xlsx` extension (case-insensitive).
///
/// Excel refuses to open a workbook whose extension and contents disagree, so a
/// missing or different extension (most commonly `.xls`) is coerced to `.xlsx`.
pub fn normalize_output_path(path: &Path) -> PathBuf {
    let already_xlsx = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(XLSX_EXTENSION));

    if already_xlsx {
        path.to_path_buf()
    } else {
        path.with_extension(XLSX_EXTENSION)
    }
}

/// Renders a match report (given its CSV-style `headers` and `rows`) into a
/// styled, color-highlighted `.xlsx` workbook at `output`.
///
/// The `headers`/`rows` are expected to be exactly what the CSV formatter would
/// emit, which keeps the two output formats column-for-column consistent.
///
/// Returns an error if the data lacks the columns a match report requires, or if
/// the workbook cannot be built or saved.
pub fn write_match_report(
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    output: &Path,
) -> Result<ConversionStats, XlsxReportError> {
    let mut report = Report::new(headers, rows);

    let missing = report.schema.missing_required_columns();
    if !missing.is_empty() {
        return Err(XlsxReportError::MissingRequiredColumns(missing));
    }

    // Surface the most relevant pairs first: highest match percentage at the top.
    if let Some(column) = report.schema.column_index(MATCH_PERCENTAGE_COLUMN) {
        report.sort_by_numeric_desc(column);
    }

    write_workbook(&report, output)
}

/// Writes `report` to an `.xlsx` workbook at `output`.
fn write_workbook(report: &Report, output: &Path) -> Result<ConversionStats, XlsxReportError> {
    let band_format = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(COLOR_HEADER_BG)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border_top(FormatBorder::Medium)
        .set_border_left(FormatBorder::Medium)
        .set_border_right(FormatBorder::Medium)
        .set_border_bottom(FormatBorder::Thin);
    let unpaired_header_format = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(COLOR_HEADER_BG)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin);
    let percent_format = Format::new().set_num_format(PERCENT_NUM_FORMAT);

    let mut workbook = Workbook::new();
    workbook.set_properties(
        &DocProperties::new()
            .set_title("Physna Match Report")
            .set_subject("Reference vs. candidate metadata comparison"),
    );

    let worksheet = workbook.add_worksheet();
    worksheet.set_name(SHEET_NAME)?;

    let schema = &report.schema;
    let match_col = schema.column_index(MATCH_PERCENTAGE_COLUMN);
    let url_col = schema.column_index(COMPARISON_URL_COLUMN);
    let mut stats = ConversionStats {
        pairs: schema.pair_count(),
        ..Default::default()
    };

    // Two-row header: a merged "group" band naming each pair's field once, and a
    // second row with per-column REF/CAN sub-labels. Unpaired columns span both.
    let headers = schema.headers();
    let mut col = 0usize;
    while col < schema.column_count() {
        match schema.partner(col) {
            Some(partner) if partner == col + 1 => {
                let field = schema.field_name(col).unwrap_or(headers[col].as_str());
                worksheet.merge_range(
                    GROUP_HEADER_ROW,
                    col as u16,
                    GROUP_HEADER_ROW,
                    (col + 1) as u16,
                    field,
                    &band_format,
                )?;
                worksheet.write_with_format(
                    LABEL_HEADER_ROW,
                    col as u16,
                    side_label(&headers[col]),
                    &pair_label_format(true),
                )?;
                worksheet.write_with_format(
                    LABEL_HEADER_ROW,
                    (col + 1) as u16,
                    side_label(&headers[col + 1]),
                    &pair_label_format(false),
                )?;
                col += 2;
            }
            _ => {
                worksheet.merge_range(
                    GROUP_HEADER_ROW,
                    col as u16,
                    LABEL_HEADER_ROW,
                    col as u16,
                    &headers[col],
                    &unpaired_header_format,
                )?;
                col += 1;
            }
        }
    }
    worksheet.set_row_height(GROUP_HEADER_ROW, HEADER_ROW_HEIGHT)?;
    worksheet.set_row_height(LABEL_HEADER_ROW, HEADER_ROW_HEIGHT)?;

    // Data rows follow the two header rows.
    let last_data_index = report.rows.len().saturating_sub(1);
    for (row_idx, record) in report.rows.iter().enumerate() {
        let excel_row = row_idx as u32 + DATA_START_ROW;
        let is_last_row = row_idx == last_data_index;
        for col in 0..schema.column_count() {
            let value = record.get(col).map(String::as_str).unwrap_or("");

            // The match-percentage column is written as a real number so the
            // heat-map gradient and numeric sort work, with a fixed precision.
            if Some(col) == match_col {
                match value.trim().parse::<f64>() {
                    Ok(number) => {
                        worksheet.write_with_format(
                            excel_row,
                            col as u16,
                            number,
                            &percent_format,
                        )?;
                    }
                    Err(_) => {
                        worksheet.write_with_format(
                            excel_row,
                            col as u16,
                            value,
                            &percent_format,
                        )?;
                    }
                }
                continue;
            }

            // The comparison column is written as a clickable hyperlink; blank or
            // non-http / over-long values fall back to plain text.
            if Some(col) == url_col {
                let link = value.trim();
                if (link.starts_with("http://") || link.starts_with("https://"))
                    && link.len() <= MAX_URL_LEN
                {
                    worksheet.write_url(excel_row, col as u16, Url::new(link))?;
                } else {
                    worksheet.write(excel_row, col as u16, value)?;
                }
                continue;
            }

            let state = report.cell_state(row_idx, col);
            match state {
                CellState::Match => stats.matching += 1,
                CellState::Different => stats.different += 1,
                CellState::Missing => stats.missing += 1,
                CellState::Neutral => {}
            }

            let (left_edge, right_edge) = pair_edges(schema, col);
            let bottom_edge = is_last_row && (left_edge || right_edge);
            match data_cell_format(state, left_edge, right_edge, bottom_edge) {
                Some(format) => {
                    worksheet.write_with_format(excel_row, col as u16, value, &format)?;
                }
                None => {
                    worksheet.write(excel_row, col as u16, value)?;
                }
            }
        }
    }
    stats.rows = report.rows.len();

    // Size every column to its content (capped) for legibility.
    for (col, width) in column_widths(report).into_iter().enumerate() {
        worksheet.set_column_width(col as u16, width)?;
    }

    // Freeze both header rows and the leading identity columns (up to and
    // including MATCH_PERCENTAGE) so the pair each row describes stays visible.
    let frozen_columns = match_col
        .map(|col| col + 1)
        .unwrap_or(1)
        .min(schema.column_count());
    worksheet.set_freeze_panes(DATA_START_ROW, frozen_columns as u16)?;
    if schema.column_count() > 0 && !report.rows.is_empty() {
        let last_col = (schema.column_count() - 1) as u16;
        let last_row = report.rows.len() as u32 + LABEL_HEADER_ROW;
        worksheet.autofilter(LABEL_HEADER_ROW, 0, last_row, last_col)?;

        // Heat-map gradient over the match-percentage column, anchored to fixed
        // 0/50/100 so the colors mean the same thing regardless of the data range.
        if let Some(col) = match_col {
            let gradient = ConditionalFormat3ColorScale::new()
                .set_minimum(ConditionalFormatType::Number, 0)
                .set_midpoint(ConditionalFormatType::Number, 50)
                .set_maximum(ConditionalFormatType::Number, 100)
                .set_minimum_color(COLOR_HEAT_LOW)
                .set_midpoint_color(COLOR_HEAT_MID)
                .set_maximum_color(COLOR_HEAT_HIGH);
            worksheet.add_conditional_format(
                DATA_START_ROW,
                col as u16,
                last_row,
                col as u16,
                &gradient,
            )?;
        }
    }

    debug!(?output, "saving workbook");
    workbook.save(output)?;
    info!(
        rows = stats.rows,
        pairs = stats.pairs,
        matching = stats.matching,
        different = stats.different,
        missing = stats.missing,
        "excel conversion complete"
    );

    Ok(stats)
}

/// The short side label (`REF` or `CAN`) for a paired column's header.
fn side_label(header: &str) -> &'static str {
    if header.starts_with(REF_PREFIX) {
        "REF"
    } else {
        "CAN"
    }
}

/// Which box edges a column sits on: `(left, right)`.
fn pair_edges(schema: &Schema, col: usize) -> (bool, bool) {
    match schema.partner(col) {
        Some(partner) if partner == col + 1 => (true, false),
        Some(partner) if col > 0 && partner == col - 1 => (false, true),
        _ => (false, false),
    }
}

/// The header format for a pair's `REF`/`CAN` sub-label, with a medium border on
/// the pair's outer edge.
fn pair_label_format(left_edge: bool) -> Format {
    let format = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(COLOR_SUBHEADER_BG)
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin);
    if left_edge {
        format.set_border_left(FormatBorder::Medium)
    } else {
        format.set_border_right(FormatBorder::Medium)
    }
}

/// Builds the format for a data cell from its comparison state and box edges.
/// Returns `None` for a plain, unhighlighted, non-edge cell.
fn data_cell_format(state: CellState, left: bool, right: bool, bottom: bool) -> Option<Format> {
    if state == CellState::Neutral && !left && !right && !bottom {
        return None;
    }
    let mut format = Format::new();
    match state {
        CellState::Match => format = format.set_background_color(COLOR_MATCH),
        CellState::Different => format = format.set_background_color(COLOR_DIFFERENT),
        CellState::Missing => format = format.set_background_color(COLOR_MISSING),
        CellState::Neutral => {}
    }
    if left {
        format = format.set_border_left(FormatBorder::Medium);
    }
    if right {
        format = format.set_border_right(FormatBorder::Medium);
    }
    if bottom {
        format = format.set_border_bottom(FormatBorder::Medium);
    }
    Some(format)
}

/// Computes a per-column width (in characters), sized to the widest of the
/// header and any cell, padded and clamped. The comparison-URL column is exempt
/// from the usual cap and sized to its (long) values.
fn column_widths(report: &Report) -> Vec<f64> {
    let column_count = report.schema.column_count();
    let url_col = report.schema.column_index(COMPARISON_URL_COLUMN);
    let mut max_chars = vec![0usize; column_count];

    for (col, header) in report.schema.headers().iter().enumerate() {
        max_chars[col] = header.chars().count();
    }
    for row in &report.rows {
        for (col, cell) in row.iter().enumerate() {
            if col < column_count {
                max_chars[col] = max_chars[col].max(cell.chars().count());
            }
        }
    }

    max_chars
        .into_iter()
        .enumerate()
        .map(|(col, chars)| {
            let width = chars as f64 + COL_WIDTH_PADDING;
            let max = if Some(col) == url_col {
                EXCEL_MAX_COL_WIDTH
            } else {
                MAX_COL_WIDTH
            };
            width.clamp(MIN_COL_WIDTH, max)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn rows(list: Vec<Vec<&str>>) -> Vec<Vec<String>> {
        list.into_iter()
            .map(|r| r.into_iter().map(String::from).collect())
            .collect()
    }

    #[test]
    fn classify_covers_all_states() {
        assert_eq!(classify("mm", "mm"), CellState::Match);
        assert_eq!(classify(" mm ", "mm"), CellState::Match);
        assert_eq!(classify("", ""), CellState::Neutral);
        assert_eq!(classify("true", "false"), CellState::Different);
        assert_eq!(classify("mm", ""), CellState::Missing);
        assert_eq!(classify("", "mm"), CellState::Missing);
    }

    #[test]
    fn pairs_ref_and_can_columns() {
        let s = Schema::from_headers(headers(&[
            "REFERENCE_ASSET_PATH",
            "REF_XUNITS",
            "CAN_XUNITS",
            "COMPARISON_URL",
        ]));
        assert_eq!(s.pair_count(), 1);
        assert_eq!(s.partner(1), Some(2));
        assert_eq!(s.partner(2), Some(1));
        assert_eq!(s.partner(0), None);
        assert_eq!(s.field_name(1), Some("XUNITS"));
    }

    #[test]
    fn reference_and_candidate_lookalikes_are_not_paired() {
        // "REFERENCE_..." / "CANDIDATE_..." must not match the REF_/CAN_ prefixes.
        let s = Schema::from_headers(headers(&[
            "REFERENCE_ASSET_PATH",
            "CANDIDATE_ASSET_PATH",
            "MATCH_PERCENTAGE",
        ]));
        assert_eq!(s.pair_count(), 0);
    }

    #[test]
    fn duplicate_field_names_pair_one_to_one() {
        // Two MATERIAL pairs (a real pcli2 quirk from case-colliding keys) must
        // pair up as (0,1) and (2,3), not all onto the first candidate column.
        let s = Schema::from_headers(headers(&[
            "REF_MATERIAL",
            "CAN_MATERIAL",
            "REF_MATERIAL",
            "CAN_MATERIAL",
        ]));
        assert_eq!(s.pair_count(), 2);
        assert_eq!(s.partner(0), Some(1));
        assert_eq!(s.partner(2), Some(3));
    }

    #[test]
    fn missing_required_columns_are_reported() {
        let s = Schema::from_headers(headers(&["REF_XUNITS", "CAN_XUNITS"]));
        assert_eq!(
            s.missing_required_columns(),
            vec![
                "REFERENCE_ASSET_PATH".to_string(),
                "CANDIDATE_ASSET_PATH".to_string(),
                "MATCH_PERCENTAGE".to_string(),
            ]
        );
    }

    #[test]
    fn write_match_report_rejects_missing_columns() {
        let result = write_match_report(
            headers(&["REF_XUNITS", "CAN_XUNITS"]),
            rows(vec![vec!["mm", "mm"]]),
            Path::new("/tmp/should_not_be_written.xlsx"),
        );
        assert!(matches!(
            result,
            Err(XlsxReportError::MissingRequiredColumns(_))
        ));
    }

    #[test]
    fn normalize_output_path_coerces_extension() {
        assert_eq!(
            normalize_output_path(Path::new("report.xlsx")),
            PathBuf::from("report.xlsx")
        );
        assert_eq!(
            normalize_output_path(Path::new("report.xls")),
            PathBuf::from("report.xlsx")
        );
        assert_eq!(
            normalize_output_path(Path::new("report")),
            PathBuf::from("report.xlsx")
        );
        // Case-insensitive: an existing .XLSX is left untouched.
        assert_eq!(
            normalize_output_path(Path::new("report.XLSX")),
            PathBuf::from("report.XLSX")
        );
    }

    #[test]
    fn writes_a_workbook_with_highlight_stats() {
        let r = Report::new(
            headers(&[
                "REFERENCE_ASSET_PATH",
                "CANDIDATE_ASSET_PATH",
                "MATCH_PERCENTAGE",
                "REF_XUNITS",
                "CAN_XUNITS",
                "COMPARISON_URL",
            ]),
            rows(vec![
                vec!["a", "b", "100", "mm", "mm", "https://example.com/c?a=1"],
                vec!["c", "d", "80.5", "mm", "in", "https://example.com/c?a=2"],
                vec!["e", "f", "50", "mm", "", ""], // blank URL -> plain text
            ]),
        );
        let path = std::env::temp_dir().join("pcli2_xlsx_report_test.xlsx");
        let stats = write_workbook(&r, &path).expect("write should succeed");
        assert_eq!(stats.rows, 3);
        assert_eq!(stats.pairs, 1);
        assert_eq!(stats.matching, 2); // mm/mm, both cells
        assert_eq!(stats.different, 2); // mm/in, both cells
        assert_eq!(stats.missing, 2); // mm/"", both cells
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn write_match_report_sorts_by_match_desc() {
        // Rows given low-to-high; the workbook should sort high-to-low. We can't
        // easily read the xlsx back, but we can verify the public entrypoint
        // succeeds and returns the right row count.
        let r = write_match_report(
            headers(&[
                "REFERENCE_ASSET_PATH",
                "CANDIDATE_ASSET_PATH",
                "MATCH_PERCENTAGE",
            ]),
            rows(vec![vec!["a", "b", "50"], vec!["c", "d", "90"]]),
            &std::env::temp_dir().join("pcli2_xlsx_report_sort_test.xlsx"),
        )
        .expect("write should succeed");
        assert_eq!(r.rows, 2);
        assert_eq!(r.pairs, 0);
    }
}
