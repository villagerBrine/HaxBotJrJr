//! Provides [`Pager`] and [`ToPage`] for lazy page generation.
//!
//! [`ToPage`] is a conversion trait, that converts the implemented type into [`ToPage::Page`] with
//! additional page info using [`ToPage.to_page`].
//! ```
//! use msgtool::pager::ToPage;
//!
//! struct UserData {
//!     name: String,
//!     age: u64,
//! }
//!
//! impl ToPage for UserData {
//!     type Page = String;
//!
//!     fn to_page(&self, page_info: Option<(usize, usize)>) -> Self::Page {
//!         let mut description = format!("Name is {}, {} years old", self.name, self.age);
//!         if let Some((current_index, total)) = page_info {
//!             description.push_str(&format!(" ({}/{})", current_index, total));
//!         }
//!         description
//!     }
//! }
//! ```
//! [`Pager`] is a tool for lazy page generation using [`ToPage`].
//! It is created by giving it the page data, and you can get the page using [`Pager.get_page`].
//! The important thing about [`Pager`] is that it is lazy, meaning it only generates the page
//! using [`ToPage.to_page`] when [`Pager.get_page`] is called (mostly true), and all generated
//! pages are cached and reused.
//! ```
//! use msgtool::pager::{Pager, ToPage};
//!
//! struct StrWrapper<'a>(&'a str);
//!
//! impl<'a> ToPage for StrWrapper<'a> {
//!     type Page = String;
//!
//!     fn to_page(&self, page_info: Option<(usize, usize)>) -> Self::Page {
//!         let mut uppercase = self.0.to_ascii_uppercase();
//!         if let Some((index, _)) = page_info {
//!             uppercase.push_str(&format!("({})", index));
//!         }
//!         uppercase
//!     }
//! }
//!
//! let data = vec![StrWrapper("foo"), StrWrapper("bar"), StrWrapper("test")];
//! let mut pager: Pager<StrWrapper, String> = Pager::new(data);
//!
//! assert!(pager.get_page() == &String::from("FOO(1)"));
//! pager.next();
//! assert!(pager.get_page() == &String::from("BAR(2)"));
//! pager.prev();
//! assert!(pager.get_page() == &String::from("FOO(1)"));
//! pager.last();
//! assert!(pager.get_page() == &String::from("TEST(3)"));
//! pager.next();
//! assert!(pager.get_page() == &String::from("FOO(1)"));
//! ```

/// Trait for page data that can be converted to page of type [`ToPage::Page`]
pub trait ToPage {
    type Page;

    /// Create a page data.
    /// `page_info` is a tuple of current page index and total number of pages in that order
    fn to_page(&self, page_info: Option<(usize, usize)>) -> Self::Page;
}

#[derive(Debug)]
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
    /// Create a new pager with provided page data
    ///
    /// The first page is generated here, so that [`Pager.get_page`] don't have to use `&mut self`.
    ///
    /// # Panic
    /// Panics if the page data vector is empty.
    pub fn new(data: Vec<D>) -> Pager<D, P> {
        if data.is_empty() {
            panic!("Empty pager data")
        }
        let pages = Vec::with_capacity(data.len());
        let mut pager = Self { data, index: 0, pages, is_full: false };
        pager.pages.push(Some(pager.make_page()));
        pager
    }

    /// Generate the current page according to the index
    fn make_page(&self) -> P {
        self.data
            .get(self.index)
            .unwrap()
            .to_page(Some((self.index + 1, self.data.len())))
    }

    /// Generate the current page if haven't, otherwise do nothing
    fn try_add_page(&mut self) {
        match self.pages.get(self.index) {
            // insert new page
            None => {
                let p = self.make_page();
                self.insert_page(p);
            }
            // replace filler with new page
            Some(None) => {
                let p = self.make_page();
                self.pages.remove(self.index);
                self.insert_page(p);
            }
            _ => {}
        }
    }

    /// Insert a page into current page index
    fn insert_page(&mut self, page: P) {
        // If page index is out of bound, filler items `None` are inserted to pad it out
        if self.index > self.pages.len() {
            for _ in 0..self.index - self.pages.len() {
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
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Get the page at the page index
    pub fn get_page(&self) -> &P {
        if let Some(p) = self.pages.get(self.index).unwrap() {
            return p;
        }
        panic!("No page at page index")
    }
}
