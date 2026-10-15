#![allow(unused)]
#![allow(test_only_dead_code)]
#![allow(dead_code)]

// Regression UI: a body-less `pub mod` declaration acts as an import for
// grouping purposes, so a `pub use` directly after it must NOT be flagged.
// A misplaced `use` with a nested group (`{Crc32, crc32}`) must be flagged
// only ONCE even though rustc lowers the tree to several HIR items.

pub mod checksum;
pub use checksum::{Crc32, crc32};

pub struct First;

pub use checksum::Extra;

// 嵌套 use 树位于非 use 项之后：rustc 展开为多个 HIR 项，必须只报一次
pub use checksum::{Crc32 as C1, crc32 as c2};

fn main() {}
