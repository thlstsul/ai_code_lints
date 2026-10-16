// 完整性 UI 测试：覆盖 fully_qualified_path_without_use 的触发/不触发形态。
// 注意：本目录文件被 ui_test 框架按二进制 crate 编译，必须带 fn main()。
#![allow(test_only_dead_code)]
#![allow(unused)]
#![allow(dead_code)]

use crate::util::helper2;
use std::cell::RefCell;
use std::collections::HashMap as Map;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug as FmtDebug;

pub mod util {
    pub fn helper() {}
    pub fn helper2() {}
}

// ---------------------------------------------------------------------------
// 应触发：表达式中的全限定路径
// ---------------------------------------------------------------------------

pub fn direct_expr() {
    crate::util::helper();
    std::mem::drop(0);
}

pub fn via_import() {
    helper2();
}

// ---------------------------------------------------------------------------
// 应触发：返回类型 / 字段类型中的全限定路径（历史漏报形态）
// ---------------------------------------------------------------------------

// 单元泛型：无空格
pub fn unit_generic() -> std::io::Result<()> {
    Ok(())
}

// 多元素元组泛型：含 ", "（空格），曾被空白守卫误杀
pub fn multi_elem_tuple() -> std::io::Result<(u32, String)> {
    Ok((0, String::new()))
}

// 双泛型参数：`Result<T, E>` 形态，含空格
pub fn two_generic_args() -> std::result::Result<u32, std::io::Error> {
    Ok(0)
}

// 嵌套泛型：泛型参数内部还有 "::" 路径
pub fn nested_generic() -> std::io::Result<std::io::Error> {
    Err(std::io::Error::other("x"))
}

// 嵌套泛型 + 多参数 + 空格：三层叠加
pub fn nested_multi_generic(
) -> std::result::Result<Vec<(std::string::String, u32)>, std::io::Error> {
    Ok(Vec::new())
}

// 泛型参数中的 trait 对象
pub fn trait_object_generic() -> std::result::Result<Box<dyn std::fmt::Debug>, u32> {
    Ok(Box::new(0))
}

// 无泛型的纯路径返回类型
pub fn plain_path_return() -> std::io::Error {
    std::io::Error::other("x")
}

// 引用参数 + 路径返回类型
pub fn ref_return(base: &str) -> std::io::Error {
    let _ = base;
    std::io::Error::other("x")
}

pub struct InFieldType {
    field: std::io::Error,
    nested: std::cell::RefCell<Option<std::io::Error>>,
}

// impl 块内的返回类型与字段表达式
pub struct Wrapper(std::io::Error);

impl Wrapper {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self(std::io::Error::other("x")))
    }
}

// 泛型函数签名中的路径
pub fn generic_fn<T: std::fmt::Debug>(t: T) -> std::result::Result<T, std::io::Error> {
    Ok(t)
}

// where 子句中的路径
pub fn where_clause<T>(t: T) -> std::result::Result<T, std::io::Error>
where
    T: std::fmt::Debug,
{
    Ok(t)
}

// 表达式中的路径 + turbofish
pub fn turbofish() -> std::result::Result<u32, std::io::Error> {
    "1".parse::<u32>().map_err(std::io::Error::other)
}

// ---------------------------------------------------------------------------
// 不应触发：类型别名定义（别名定义处不是调用点，且按建议改成 use 会与
// 别名自身命名冲突）
// ---------------------------------------------------------------------------

pub type ResultOf<T> = std::result::Result<T, std::io::Error>;

pub type PlainAlias = std::io::Error;

// ---------------------------------------------------------------------------
// 不应触发：use 语句（含别名 / 下划线导入 / 嵌套 use 树）、
// 单段路径、crate:: 单段、宏展开生成的路径
// ---------------------------------------------------------------------------

pub fn use_imported() {
    let _: Map<u32, u32> = Map::new();
    let _: RefCell<u32> = RefCell::new(0);
    let _: BTreeMap<u32, u32> = BTreeMap::new();
    let _: BTreeSet<u32> = BTreeSet::new();
    let _: pub_helper::Alias = pub_helper::Alias;

    fn assert_debug<T: FmtDebug>() {}
    assert_debug::<u32>();
}

pub mod pub_helper {
    pub struct Alias;
}

// 单段路径（首段非 crate root）不应触发
pub fn local_types() -> Option<String> {
    None
}

// crate:: + 单段标识符：应触发（crate:: 后至少一段即算全限定）
pub fn crate_self_path() -> u32 {
    crate::CONST_ITEM
}

pub const CONST_ITEM: u32 = 0;

// 宏展开生成的路径不应触发（span 语法上下文属于宏，非用户手写）
macro_rules! make_fn {
    () => {
        pub fn generated_by_macro() -> std::io::Result<()> {
            Ok(())
        }
    };
}

make_fn!();

fn main() {}
