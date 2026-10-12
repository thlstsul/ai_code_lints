#![allow(unused)]

pub use inner::S;
pub mod inner {
    pub struct S;
}

pub mod widget {
    pub struct Widget {
        pub name: String,
    }
    impl Widget {
        pub fn helper(&self) {}
    }
    pub fn dead_helper() {}
}

#[cfg(test)]
mod tests {
    use super::inner::S as InnerS;
    use super::widget::{Widget, dead_helper};

    #[test]
    fn uses_things() {
        let w = Widget { name: String::new() };
        w.helper();
        dead_helper();
        let _ = InnerS;
    }
}
