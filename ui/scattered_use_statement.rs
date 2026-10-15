// 完整性 UI 测试：覆盖 scattered_use_statement 的触发/不触发形态。
// harness 按二进制 crate 编译，必须带 fn main()。
#![allow(unused)]
#![allow(test_only_dead_code)]
#![allow(dead_code)]
#![allow(unused_extern_crates)]

// ---------------------------------------------------------------------------
// 不应触发：模块头部的 use（含别名、嵌套 use 树）、extern crate、
// 紧跟 body-less `mod x;` 之后的 use（它本身按 import 分组）
// ---------------------------------------------------------------------------

use std::fmt::Debug as FmtDebug;
use std::collections::{BTreeMap, BTreeSet};

extern crate std as std_crate;

// body-less `mod x;` 只命名文件，按 import 分组：紧随其后的 use 不触发
pub mod checksum;
use checksum::Crc32;

// ---------------------------------------------------------------------------
// 应触发：位于非 use 项之后的 use
// ---------------------------------------------------------------------------

pub struct After;

// 位于 struct 之后：触发
use std::fmt::Display as _;

// 每条位于非 use 项之后的 use 都会逐条标记（去重只针对同一棵 use 树的叶子）
use std::fmt::Write as _;

// 内联 `mod m { }` 不是 import-like：位于其后的 use 触发
mod inline_mod {
    pub struct Inner;
}

use std::io;

// ---------------------------------------------------------------------------
// 应触发：嵌套 use 树位于非 use 项之后 —— rustc 会把树展开为多个 HIR 项，
// 必须只报一次（span 去重）
// ---------------------------------------------------------------------------

pub struct After2;

use checksum::{Crc32 as C1, crc32 as c2};

// ---------------------------------------------------------------------------
// 应触发：函数体内 / 子模块非头部的 use
// ---------------------------------------------------------------------------

pub fn in_fn_body() {
    use std::collections::HashMap;
    let _: HashMap<u32, u32> = HashMap::new();
}

pub mod nested {
    pub struct Deep;
    // 位于子模块内非 use 项之后：触发
    use std::rc::Rc;
}

pub fn uses_fmt<T: FmtDebug>(t: T) {
    let _ = t;
}

fn main() {}
