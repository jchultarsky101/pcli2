//! When to stop walking a paginated listing.
//!
//! The API pages every listing. The client had fourteen loops over those pages,
//! and they had drifted apart: some stopped when the server echoed a page number
//! that did not advance and some did not (the folder tree, which every path lookup
//! reads, would ask for the same page forever and append its folders again each
//! time); some capped the walk and said so, others capped it without a word and
//! returned a partial result as if it were complete. The rules live here now.

use tracing::warn;

/// A listing is never walked past this many pages. At the page sizes the client
/// uses that is hundreds of thousands of records, far more than any listing
/// should hold; reaching it means the server is misbehaving, and the partial
/// result is reported as such.
pub const MAX_PAGES: usize = 1000;

/// Tracks the page to ask for next and decides, after each page, whether to go on.
#[derive(Debug)]
pub struct Pager {
    next: usize,
    what: &'static str,
    max_pages: usize,
}

impl Pager {
    /// Start at page 1. `what` names the listing in warnings ("folder listing").
    pub fn new(what: &'static str) -> Self {
        Self {
            next: 1,
            what,
            max_pages: MAX_PAGES,
        }
    }

    /// Start at page 1 with a different page cap, for a listing that can
    /// legitimately be larger than [`MAX_PAGES`] pages.
    pub fn with_max_pages(what: &'static str, max_pages: usize) -> Self {
        Self {
            next: 1,
            what,
            max_pages,
        }
    }

    /// The page to request.
    pub fn page(&self) -> usize {
        self.next
    }

    /// Record the page that just arrived and say whether to fetch another.
    ///
    /// Stops, quietly, after the last page. Stops with a warning when the server
    /// answers with a page number below the one asked for (a stale page that would
    /// otherwise be requested forever), or when the page cap is reached, since
    /// either way the result may be incomplete. `collected` is how many records
    /// the caller holds so far, for the warning.
    pub fn advance(&mut self, current_page: usize, last_page: usize, collected: usize) -> bool {
        let requested = self.next;
        if current_page >= last_page {
            return false;
        }
        if current_page < requested {
            warn!(
                "The {} returned page {} when page {} was requested; stopping with {} record(s), which may be incomplete",
                self.what, current_page, requested, collected
            );
            return false;
        }
        if requested >= self.max_pages {
            warn!(
                "The {} has more than {} pages; stopping with {} record(s), so the result is incomplete",
                self.what, self.max_pages, collected
            );
            return false;
        }
        self.next = current_page + 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_until_the_last_page() {
        let mut pager = Pager::new("test listing");
        assert_eq!(pager.page(), 1);
        assert!(pager.advance(1, 3, 10));
        assert_eq!(pager.page(), 2);
        assert!(pager.advance(2, 3, 20));
        assert!(!pager.advance(3, 3, 30));
    }

    #[test]
    fn a_stale_page_number_stops_the_walk() {
        let mut pager = Pager::new("test listing");
        assert!(pager.advance(1, 5, 10));
        // Asked for 2, got 1 again.
        assert!(!pager.advance(1, 5, 20));
    }

    #[test]
    fn the_page_cap_stops_the_walk() {
        let mut pager = Pager {
            next: 1,
            what: "test listing",
            max_pages: 2,
        };
        assert!(pager.advance(1, 10, 10));
        assert!(!pager.advance(2, 10, 20));
    }

    #[test]
    fn a_single_page_listing_stops_at_once() {
        let mut pager = Pager::new("test listing");
        assert!(!pager.advance(1, 1, 3));
        // An empty listing reports last page 0.
        let mut empty = Pager::new("test listing");
        assert!(!empty.advance(1, 0, 0));
    }
}
