#![allow(test_only_dead_code)]
#![allow(unused)]

pub struct Alpha;

pub fn f(a: Alpha) -> Alpha {
    a
}

use crate::Alpha as A;

pub struct Beta;

pub fn g(b: Beta) -> Beta {
    b
}

pub fn use_a(a: A) -> A {
    a
}
