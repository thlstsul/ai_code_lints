#![allow(test_only_dead_code)]
#![allow(unused)]

use crate::util::helper2;

pub mod util {
    pub fn helper() {}
    pub fn helper2() {}
}

pub fn direct() {
    crate::util::helper();
    std::mem::drop(0);
}

pub fn via_import() {
    helper2();
}
