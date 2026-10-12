#![feature(rustc_private)]
#![warn(unused_extern_crates)]

// A list of available compiler crates can be found here:
// https://doc.rust-lang.org/nightly/nightly-rustc/
extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use rustc_ast::attr::AttributeExt;
use rustc_span::sym;

use rustc_lint::LintStore;
use rustc_session::Session;

use dylint_linting::init_config;

dylint_linting::dylint_library!();

mod fully_qualified_path;
mod scattered_use_statement;
mod test_only_dead_code;

/// `#[test]` 或 `#[cfg(test)]` 属性。
pub fn is_test_cfg_attr<A: AttributeExt>(attr: &A) -> bool {
    attr.has_name(sym::test)
        || (attr.path_matches(&[sym::cfg])
            && attr.meta_item_list().is_some_and(|list| {
                list.iter().any(|item| {
                    item.meta_item()
                        .is_some_and(|meta| meta.path.segments.len() == 1 && meta.path.segments[0].ident.name == sym::test)
                })
            }))
}

#[unsafe(no_mangle)]
pub fn register_lints(sess: &Session, lint_store: &mut LintStore) {
    init_config(sess);
    lint_store.register_lints(&[
        test_only_dead_code::TEST_ONLY_DEAD_CODE,
        scattered_use_statement::SCATTERED_USE_STATEMENT,
        fully_qualified_path::FULLY_QUALIFIED_PATH_WITHOUT_USE,
    ]);
    lint_store.register_late_lint_pass(Box::new(|_| Box::new(test_only_dead_code::TestOnlyDeadCode::default())));
    lint_store.register_late_lint_pass(Box::new(|_| Box::new(scattered_use_statement::ScatteredUseStatement::default())));
    lint_store.register_late_lint_pass(Box::new(|_| Box::new(fully_qualified_path::FullyQualifiedPathWithoutUse::default())));
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
