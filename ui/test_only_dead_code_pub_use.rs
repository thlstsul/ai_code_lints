#![allow(unused)]

// Regression UI for TEST_ONLY_DEAD_CODE: a body-less `pub mod` declaration
// followed by `pub use` re-exports must NOT be flagged — a public re-export
// is a deliberate part of the crate's API surface, and the intermediate
// module segment counts as used.

pub mod checksum;
pub use checksum::{Crc32, crc32};

// Still flagged: only referenced from test code.
pub mod widget {
    pub fn dead_helper() {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn t() {
        super::widget::dead_helper();
    }
}
