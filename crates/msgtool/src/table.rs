//! Utilities for formatting tables
//!
//! This module provides the function [`format_table`] for formatting a 2d vector into a table.
//! Functions that formats part of a table are also provided.
//!
//! The provided struct [`TableData`] allows for table pagination, and implements [`ToPage`] which
//! uses [`format_table`], so it can also be used with [`Pager`].
//!
//! This module uses `Vec<Vec<&str>>` to represent table, and provides [`borrow_table`] and
//! [`borrow_row`] for conversion from `Vec<Vec<String>>` to `Vec<Vec<&str>>`.
//!
//! [`ToPage`]: crate::pager::ToPage
//! [`Pager`]: crate::pager::Pager
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

/// Format a 2d string vector into a fancy table.
///
/// If `page_info` is provided then a page index indicator is included.
/// `page_info` is a tuple of current page index and total number of pages in that order.
/// ```
/// use msgtool::table::format_table;
///
/// let mut table = Vec::new();
///
/// assert!(format_table(&table, Some((1, 3))).is_empty());
///
/// table.push(vec!["name", "rank"      , "xp"]);
///
/// assert!(format_table(&table, None) ==
///"╭────┬────┬──╮
/// │name│rank│xp│
/// ╞════╪════╪══╡
/// ╰────┴────┴──╯");
///
/// table.push(vec!["foo" , "Owner"     , "10M"]);
/// table.push(vec!["bar" , "Strategist", "15B"]);
/// table.push(vec!["test", "Captain"   , "10,200"]);
///
///
/// assert!(format_table(&table, Some((1, 3))) ==
///"╭────┬──────────┬──────╮
/// │name│rank      │xp    │
/// ╞════╪══════════╪══════╡
/// │foo │Owner     │10M   │
/// │bar │Strategist│15B   │
/// │test│Captain   │10,200│
/// ╰────┴──────────┴──────╯
///  ● ◌ ◌");
/// ```
pub fn format_table(table: &Vec<Vec<&str>>, page_info: Option<(usize, usize)>) -> String {
    if table.is_empty() {
        return String::new();
    }

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

/// Convert a 2d vector of string to 2d vector of borrowed string
pub fn borrow_table(table: &[Vec<String>]) -> Vec<Vec<&str>> {
    table.iter().map(|row| borrow_row(row)).collect()
}

/// Convert a vector of string to vector of borrowed string
pub fn borrow_row(row: &[String]) -> Vec<&str> {
    row.iter().map(|s| s.as_str()).collect()
}

/// Get the max width of each columns of a 2d string vector.
///
/// The returned vector corresponds to each columns in order.
/// ```
/// use msgtool::table::calc_cols_max_width;
///
/// let mut table = Vec::new();
/// table.push(vec!["name", "rank"      , "xp"]);
/// table.push(vec!["foo" , "Owner"     , "10M"]);
/// table.push(vec!["bar" , "Strategist", "15B"]);
/// table.push(vec!["test", "Captain"   , "10,200"]);
///
/// assert!(calc_cols_max_width(&table) == vec![4, 10, 6]);
/// ```
pub fn calc_cols_max_width(table: &Vec<Vec<&str>>) -> Vec<usize> {
    if table.is_empty() {
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

/// Format a string vector into table row.
///
/// `widths` is the widths of each columns in order, which canbe calculated using
/// [`calc_cols_max_width`].
/// if a string is smaller than its corresponding column width, it is then padded with space and
/// left aligned.
///
/// Note that a new line is inserted at the beginning of the row.
/// ```
/// use msgtool::table::format_row;
///
/// let row = vec!["foo" , "Owner", "10M"];
/// let widths = vec![4, 10, 6];
/// assert!(format_row(&row, &widths) == "\n│foo │Owner     │10M   │");
/// ```
pub fn format_row(row: &[&str], widths: &[usize]) -> String {
    let mut s = "\n".to_string();
    s.push(BOX_VERTICAL);

    for (i, item) in row.iter().enumerate() {
        s.push_str(item);
        // Add the padding
        if let Some(max_w) = widths.get(i) {
            s.push_str(&" ".repeat(max_w - item.len()));
        }
        s.push(BOX_VERTICAL);
    }

    s
}

/// Create a table row divider.
///
/// `widths` is the widths of each columns in order, which canbe calculated using
/// [`calc_cols_max_width`].
/// `left` is the first character, and `right` is the last character.
/// The rest is filled up with `fill`, with `div` dividing each columns according to `widths`.
/// ```
/// use msgtool::table::make_divider;
///
/// let widths = vec![4, 10, 6];
/// assert!(make_divider('<', 'o', '$', '>', &widths) == "<oooo$oooooooooo$oooooo>");
/// ```
pub fn make_divider(left: char, fill: char, div: char, right: char, widths: &Vec<usize>) -> String {
    if widths.is_empty() {
        return String::new();
    }

    let mut s = left.to_string();

    for (i, w) in widths.iter().enumerate() {
        s.push_str(&fill.to_string().repeat(*w));
        s.push(if i < widths.len() - 1 { div } else { right });
    }

    s
}

/// Create a page index indicator
///
/// Note that the page index starts at 1.
/// ```
/// use msgtool::table::make_page_indicator;
/// assert!(make_page_indicator(1, 1) == "●");
/// assert!(make_page_indicator(2, 3) == "◌ ● ◌")
/// ```
///
/// # Panic
/// Panics if `page_index` is greater than `page_num`
pub fn make_page_indicator(page_index: usize, page_num: usize) -> String {
    if page_index > page_num {
        panic!("Page index is out of bound");
    }

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
///
/// This struct is designed to be used with [`Pager`] for creating paged table.
///
/// [`Pager`]: crate::pager::Pager
#[derive(Debug, PartialEq, Eq, Default)]
pub struct TableData<'a>(pub Vec<Vec<&'a str>>);

impl<'a> TableData<'a> {
    /// Paginates a table represented by 2d string vector with header.
    ///
    /// This function splits a table into chunks of `len` rows.
    /// If `len` doesn't divide the total number of rows evenly, the last chunk will have a length
    /// of less than `len`.
    /// Each chunks also has an extra row `header` inserted as the first row.
    /// All the chunks are then wrapped in [`TableData`].
    /// ```
    /// use msgtool::table::TableData;
    ///
    /// let header = vec!["name", "rank", "xp"];
    ///
    /// let mut table = Vec::new();
    /// table.push(vec!["foo" , "Owner"     , "10M"]);
    /// table.push(vec!["bar" , "Strategist", "15B"]);
    /// table.push(vec!["test", "Captain"   , "10,200"]);
    /// table.push(vec!["abc" , "Recruit"   , "123"]);
    /// table.push(vec!["def" , "Recruiter" , "456"]);
    /// table.push(vec!["ghi" , "Chief"     , "789"]);
    /// table.push(vec!["jkl" , "Recruit"   , "0"]);
    ///
    /// let tables = TableData::paginate(table, header, 3);
    /// assert!(tables == vec![
    ///     TableData(vec![
    ///         vec!["name", "rank"      , "xp"],
    ///         vec!["foo" , "Owner"     , "10M"],
    ///         vec!["bar" , "Strategist", "15B"],
    ///         vec!["test", "Captain"   , "10,200"],
    ///     ]),
    ///     TableData(vec![
    ///         vec!["name", "rank"      , "xp"],
    ///         vec!["abc" , "Recruit"   , "123"],
    ///         vec!["def" , "Recruiter" , "456"],
    ///         vec!["ghi" , "Chief"     , "789"],
    ///     ]),
    ///     TableData(vec![
    ///         vec!["name", "rank"      , "xp"],
    ///         vec!["jkl" , "Recruit"   , "0"],
    ///     ]),
    /// ]);
    /// ```
    pub fn paginate(mut data: Vec<Vec<&'a str>>, header: Vec<&'a str>, len: usize) -> Vec<TableData<'a>> {
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

        if !data.is_empty() {
            data.insert(0, header);
            pages.push(TableData(data));
        }

        pages
    }
}

impl<'a> ToPage for TableData<'a> {
    type Page = String;

    /// Formats into table using [`format_table`].
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
