//! Utilities for formatting tables
use util::some;

use crate::pager::ToPage;

const BOX_CORNER_TL: char = '╭';
const BOX_CORNER_TR: char = '╮';
const BOX_CORNER_BL: char = '╰';
const BOX_CORNER_BR: char = '╯';
const BOX_HORIZONTAL: char = '─';
const BOX_HEADER_HORIZONTAL: char = '═';
const BOX_VERTICAL: char = '│';
const BOX_T_DOWN: char = '┬';
const BOX_T_UP: char = '┴';
const BOX_T_RIGHT: char = '╞';
const BOX_T_LEFT: char = '╡';
const BOX_CROSS: char = '╪';
const BULLET_EMPTY: char = '◌';
const BULLET_FULL: char = '●';

/// Format a 2d vec of strings into a table, and a page index indicator if `page_info` is provided.
/// `page_info` is a tuple of current page index and total number of pages.
pub fn format_table(table: &Vec<Vec<String>>, page_info: Option<(usize, usize)>) -> String {
    let max_widths = calc_cols_max_width(table);

    // Initialize the table string with the top box edge
    let mut s = make_divider(BOX_CORNER_TL, BOX_HORIZONTAL, BOX_T_DOWN, BOX_CORNER_TR, &max_widths);

    // Add the header row, aka the first row of the 2d vec
    let mut row_iter = table.iter();
    let header = some!(row_iter.next(), return String::new());
    s.push_str(&format_row(header, &max_widths));

    // Add the header divider
    s.push('\n');
    s.push_str(&make_divider(BOX_T_RIGHT, BOX_HEADER_HORIZONTAL, BOX_CROSS, BOX_T_LEFT, &max_widths));

    // Add the rest of the rows
    for row in row_iter {
        s.push_str(&format_row(row, &max_widths));
    }

    // Add the bottom box edge
    s.push('\n');
    s.push_str(&make_divider(BOX_CORNER_BL, BOX_HORIZONTAL, BOX_T_UP, BOX_CORNER_BR, &max_widths));

    if let Some((page_index, page_num)) = page_info {
        // Add the page index indicator
        s.push_str("\n ");
        s.push_str(&make_page_indicator(page_index, page_num));
    }

    s
}

/// Iterate over all table (represented as 2d vector) columns and find the max width for each of them
pub fn calc_cols_max_width(table: &Vec<Vec<String>>) -> Vec<usize> {
    if table.len() == 0 {
        return Vec::new();
    }

    let mut max_widths = Vec::new();
    for col_i in 0..table.get(0).unwrap().len() {
        let mut max_width = 0;
        for row in table {
            max_width = std::cmp::max(max_width, row.get(col_i).unwrap().len());
        }
        max_widths.push(max_width);
    }
    max_widths
}

/// Format a list of strings into a table row, padded with space based on max width of each
/// collumns
pub fn format_row(row: &Vec<String>, max_widths: &Vec<usize>) -> String {
    let mut s = "\n".to_string();
    s.push(BOX_VERTICAL);

    for (i, item) in row.iter().enumerate() {
        s.push_str(item);
        // Add the padding
        if let Some(max_w) = max_widths.get(i) {
            s.push_str(&" ".repeat(max_w - item.len()));
        }
        s.push(BOX_VERTICAL);
    }

    s
}

/// Create a table divider.
/// `left` and `right` are the edge characters, `hori` is the column dividing character, and `mid`
/// is the filler character
fn make_divider(left: char, hori: char, mid: char, right: char, max_widths: &Vec<usize>) -> String {
    let mut s = left.to_string();

    for (i, w) in max_widths.iter().enumerate() {
        s.push_str(&hori.to_string().repeat(*w));
        s.push(if i < max_widths.len() - 1 { mid } else { right });
    }

    s
}

/// Create a page index indicator
pub fn make_page_indicator(page_index: usize, page_num: usize) -> String {
    if page_index == 1 && page_num == 1 {
        return BULLET_FULL.to_string();
    }

    let mut s = if page_index == 1 { BULLET_FULL } else { BULLET_EMPTY }.to_string();

    for i in 2..page_num + 1 {
        s.push(' ');
        s.push(if i == page_index { BULLET_FULL } else { BULLET_EMPTY });
    }

    s
}

/// Data needed to create a table.
/// Can be formatted into a table via `ToPage`
#[derive(Debug)]
pub struct TableData(pub Vec<Vec<String>>);

impl TableData {
    /// Given a 2d vec of string and split it into chunks, with an optional header row added to each.
    pub fn paginate(mut data: Vec<Vec<String>>, header: Vec<String>, len: usize) -> Vec<TableData> {
        // Check if the vec is too short to be chunked
        if data.len() <= len {
            data.insert(0, header);
            return vec![TableData(data)];
        }

        let mut pages = Vec::with_capacity(data.len() / len + 1);

        while data.len() > len {
            let mut chunk = vec![header.clone()];
            for _ in 0..len {
                chunk.push(data.remove(0));
            }
            pages.push(TableData(chunk));
        }

        if data.len() != 0 {
            data.insert(0, header);
            pages.push(TableData(data));
        }

        pages
    }
}

impl ToPage for TableData {
    type Page = String;

    fn to_page(&self, page_info: Option<(usize, usize)>) -> Self::Page {
        let mut s = "```\n".to_string();
        s.push_str(&format_table(
            &self.0,
            // No page index indicator if there is only one page
            match page_info {
                Some((_, 1)) => None,
                _ => page_info,
            },
        ));
        s.push_str("\n```");
        s
    }
}
