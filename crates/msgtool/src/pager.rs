//! `Pager` is used to navigate pages (list of `P`), and can construct new pages from source `D` on
//! demand.
//! This means that `Pager`can have list of source data for generating pages from, but it will only
//! do it if that page is needed.
//!
//! It is primarily used to lazily display paged discord messages.

/// Trait for source data that can be converted to page data of type `Page`
pub trait ToPage {
    type Page;

    /// Create a page data.
    /// `page_info` is a tuple of current page index and total number of pages.
    fn to_page(&self, page_info: Option<(usize, usize)>) -> Self::Page;
}

pub struct Pager<D, P>
where
    D: ToPage<Page = P>,
{
    data: Vec<D>,
    pages: Vec<Option<P>>,
    index: usize,
    /// Became true of all page data are generated
    is_full: bool,
}

impl<D, P> Pager<D, P>
where
    D: ToPage<Page = P>,
{
    /// Create a new pager of source data
    pub fn new(data: Vec<D>) -> Pager<D, P> {
        if data.len() == 0 {
            panic!("Empty pager data")
        }
        let pages = Vec::with_capacity(data.len());
        let mut pager = Self { data, index: 0, pages, is_full: false };
        pager.pages.push(Some(pager.make_page()));
        pager
    }

    /// Create currently selected page
    fn make_page(&self) -> P {
        self.data
            .get(self.index)
            .unwrap()
            .to_page(Some((self.index + 1, self.data.len())))
    }

    /// Create currently selected page if haven't, otherwise do nothing
    fn try_add_page(&mut self) {
        match self.pages.get(self.index) {
            None => {
                let p = self.make_page();
                self.insert_page(p);
            }
            Some(None) => {
                let p = self.make_page();
                self.pages.remove(self.index);
                self.insert_page(p);
            }
            _ => {}
        }
    }

    /// Insert a page at current page index
    fn insert_page(&mut self, page: P) {
        // If page index of too large, then filler items (None) are inserted to pad it out
        if self.index > self.pages.len() {
            for _ in 0..self.data.len() - self.index {
                self.pages.push(None);
            }
        }

        self.pages.insert(self.index, Some(page));

        // Checks if all pages are generated
        if self.pages.len() == self.data.len() && !self.pages.iter().any(|p| p.is_none()) {
            self.is_full = true;
        }
    }

    /// Increment the page index
    pub fn next(&mut self) {
        self.index = if self.index == self.data.len() - 1 { 0 } else { self.index + 1 };
        if !self.is_full {
            self.try_add_page();
        }
    }

    /// Decrement the page index
    pub fn prev(&mut self) {
        self.index = if self.index == 0 { self.data.len() - 1 } else { self.index - 1 };
        if !self.is_full {
            self.try_add_page();
        }
    }

    /// Set the page index to 0
    pub fn first(&mut self) {
        self.index = 0;
    }

    /// Set the page index to last page
    pub fn last(&mut self) {
        self.index = self.data.len() - 1;
        if !self.is_full {
            self.try_add_page();
        }
    }

    /// Get the current page index
    pub fn index(&self) -> usize {
        self.index
    }

    /// Get the number of total pages
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Get the page data pointed to by page index
    pub fn get_page(&self) -> &P {
        if let Some(p) = self.pages.get(self.index).unwrap() {
            return &p;
        }
        panic!("No page at page index")
    }
}
