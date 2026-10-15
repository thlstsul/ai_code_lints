// 完整性 UI 测试：覆盖 test_only_dead_code 的触发/不触发形态。
// harness 按二进制 crate 编译（cfg(test) 被剥离）；用顶层 `#[test] fn`
// 构造"仅测试代码引用"的场景（其函数体被 lint 判定为测试上下文）。
#![allow(unused)]
#![allow(dead_code)]

// ---------------------------------------------------------------------------
// 不应触发：pub use 再导出按 API 计为真实引用（含中间段 `inner`）
// ---------------------------------------------------------------------------

pub use inner::S;
pub mod inner {
    pub struct S;
    // 二进制 crate 中完全无引用的 pub 项：触发（None → dead）
    pub struct NeverReferenced;
}

pub mod widget {
    // 被 impl 块引用（非测试代码）→ Real，不触发
    pub struct Widget {
        pub name: String,
    }
    impl Widget {
        // 关联函数不是候选项：不触发
        pub fn helper(&self) {}
    }
    // 仅被（被剥离的）cfg(test) 代码引用 → 触发
    pub fn dead_helper() {}
}

// ---------------------------------------------------------------------------
// 应触发：仅被 #[test] 函数引用的 pub 项（各类候选 kind）
// ---------------------------------------------------------------------------

pub const TEST_ONLY_CONST: u32 = 1;
pub static TEST_ONLY_STATIC: u32 = 2;
pub enum TestOnlyEnum {
    A,
}
pub trait TestOnlyTrait {}
pub type TestOnlyAlias = u32;
pub struct TestOnlyTuple(pub u32);
pub struct TestOnlyStruct {
    pub x: u32,
}

// ---------------------------------------------------------------------------
// 不应触发：同时被测试与真实代码引用（Real 优先）、#[no_mangle] 导出
// ---------------------------------------------------------------------------

pub struct UsedBoth;
pub fn real_caller() -> UsedBoth {
    UsedBoth
}

#[no_mangle]
pub extern "C" fn exported() {}

#[test]
fn test_references() {
    let _ = TEST_ONLY_CONST;
    let _ = TEST_ONLY_STATIC;
    let _ = TestOnlyEnum::A;
    let _: Box<dyn TestOnlyTrait> = Box::new(());
    let _: TestOnlyAlias = 0;
    let _ = TestOnlyTuple(1);
    let _ = TestOnlyStruct { x: 1 };
    let _ = UsedBoth;
}

#[cfg(test)]
mod tests {
    // cfg(test) 在 harness 编译下被剥离：这里的引用不参与判定，
    // 因此 widget / dead_helper 走"无引用"分支被标记。
    use super::widget::{Widget, dead_helper};

    #[test]
    fn uses_things() {
        let w = Widget {
            name: String::new(),
        };
        w.helper();
        dead_helper();
    }
}

fn main() {
    // main 中的引用按真实代码计：real_caller / UsedBoth 均不触发
    let _ = real_caller();
}
