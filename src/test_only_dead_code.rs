use std::collections::HashMap;
use std::env;
use std::fs;
use std::mem;
use std::path::Path as StdPath;

use clippy_utils::diagnostics::span_lint_and_help;
use dylint_linting::config_or_default;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{HirId, Item, ItemKind, Node, Path};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::middle::codegen_fn_attrs::CodegenFnAttrFlags;
use rustc_span::sym;

use crate::is_test_cfg_attr;

rustc_lint::declare_lint! {
    /// ### What it does
    ///
    /// Checks for public items that are only referenced by test code
    /// (`#[cfg(test)]` / `#[test]`) or not referenced at all.
    ///
    /// ### Why is this bad?
    ///
    /// rustc's dead code analysis treats all `pub` items as used because they
    /// could be exported, and any reference from test code also silences the
    /// dead code warning. An item whose only "usage" comes from tests is
    /// effectively dead in the shipped library; an item with no references
    /// at all may be dead too (unless another crate imports it).
    ///
    /// ### Known problems
    ///
    /// - In library crates, usages from other crates of the same workspace
    ///   are not visible here. Completely unreferenced items may therefore be
    ///   false positives; set `ignore_unreferenced = true` in the
    ///   `[test_only_dead_code]` table of the workspace's `dylint.toml` to
    ///   flag only items that *are* referenced (by test code or re-exports),
    ///   leaving fully unreferenced items to rustc's dead code analysis. In
    ///   binary crates nothing can be imported by another crate, so
    ///   unreferenced items are always flagged.
    /// - Items exported via `#[export_name = "..."]` are not recognized as
    ///   externally visible (only `#[no_mangle]` / `#[unsafe(no_mangle)]`
    ///   are), so they may be reported as test-only or dead.
    /// - References inside macro expansions are classified by whether the
    ///   expansion site is test code; expansions outside test code are
    ///   conservatively treated as real usages.
    ///
    /// ### Example
    ///
    /// ```rust
    /// pub mod util {
    ///     pub fn helper() {}
    /// }
    ///
    /// #[cfg(test)]
    /// mod tests {
    ///     #[test]
    ///     fn t() {
    ///         super::util::helper();
    ///     }
    /// }
    /// ```
    ///
    /// Use instead: delete `helper` (or make it non-pub so rustc can check it).
    pub TEST_ONLY_DEAD_CODE,
    Warn,
    "public item only referenced by test code (or not referenced at all)"
}

rustc_lint::impl_lint_pass!(TestOnlyDeadCode => [TEST_ONLY_DEAD_CODE]);

#[derive(Clone, Copy, PartialEq, Eq)]
enum RefKind {
    /// Referenced by non-test code (including non-test `use` re-exports,
    /// which are part of the crate's API surface).
    Real,
    /// Only referenced from `#[cfg(test)]` / `#[test]` code.
    TestOnly,
}

#[derive(Default)]
pub struct TestOnlyDeadCode {
    refs: HashMap<LocalDefId, RefKind>,
    candidates: Vec<LocalDefId>,
}

/// Configuration for `TEST_ONLY_DEAD_CODE`, read from the workspace's
/// `dylint.toml` (`[test_only_dead_code]` table).
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct Config {
    /// Usages from other crates of the same workspace are not visible here, so
    /// items with *no* reference at all may be false positives. When true,
    /// only items referenced exclusively by test code or `pub use` re-exports
    /// are flagged; completely unreferenced items are left to rustc's
    /// dead code analysis.
    ignore_unreferenced: bool,
}

fn is_test_cfg_attr_any<'tcx>(cx: &LateContext<'tcx>, hir_id: HirId) -> bool {
    cx.tcx.hir_attrs(hir_id).iter().any(is_test_cfg_attr)
}

fn is_in_test_context(cx: &LateContext<'_>, hir_id: HirId) -> bool {
    is_test_cfg_attr_any(cx, hir_id)
        || cx.tcx.hir_parent_iter(hir_id).any(
            |(_, node)| matches!(node, Node::Item(item) if is_test_cfg_attr_any(cx, item.hir_id())),
        )
}

fn classify_ref(cx: &LateContext<'_>, hir_id: HirId) -> RefKind {
    let owner = cx.tcx.hir_get_parent_item(hir_id);
    let item = match cx.tcx.hir_node_by_def_id(owner.def_id) {
        Node::Item(item) => item,
        _ => return RefKind::Real,
    };
    let in_test = is_in_test_context(cx, item.hir_id());
    if matches!(item.kind, ItemKind::Use(..)) {
        if in_test {
            RefKind::TestOnly
        } else {
            // A non-test `use` — including a public re-export like
            // `pub use submodule::Item` — is a deliberate part of the
            // crate's API surface, so it counts as a real usage.
            RefKind::Real
        }
    } else if in_test {
        RefKind::TestOnly
    } else {
        RefKind::Real
    }
}

fn is_candidate(kind: &ItemKind<'_>) -> bool {
    matches!(
        kind,
        ItemKind::Fn { .. }
            | ItemKind::Const(..)
            | ItemKind::Static(..)
            | ItemKind::Struct(..)
            | ItemKind::Union(..)
            | ItemKind::Enum(..)
            | ItemKind::Trait { .. }
            | ItemKind::TyAlias(..)
            | ItemKind::Mod(..)
    )
}

impl<'tcx> LateLintPass<'tcx> for TestOnlyDeadCode {
    fn check_path(&mut self, cx: &LateContext<'tcx>, path: &Path<'tcx>, hir_id: HirId) {
        if let Res::Def(_, def_id) = path.res {
            let mut def_id = def_id;
            // Tuple struct constructors resolve to their own def; map to the struct.
            if matches!(cx.tcx.def_kind(def_id), DefKind::Ctor(..))
                && let Some(parent) = cx.tcx.opt_parent(def_id)
            {
                def_id = parent;
            }
            if let Some(local) = def_id.as_local() {
                let owner = cx.tcx.hir_get_parent_item(hir_id);
                // Ignore references from the item's own definition body.
                if owner.def_id == local {
                    return;
                }
                // A macro-expanded path may originate from real source code
                // (e.g. `assert_eq!` expanding to a comparison). Classify it
                // by whether the expansion site is test code; references from
                // test-context expansions (e.g. `#[test]` bodies) are still
                // test-only.
                let kind = if path.span.from_expansion() {
                    if is_in_test_context(cx, hir_id) {
                        RefKind::TestOnly
                    } else {
                        RefKind::Real
                    }
                } else {
                    classify_ref(cx, hir_id)
                };
                let entry = self.refs.entry(local).or_insert(kind);
                if kind == RefKind::Real {
                    *entry = RefKind::Real;
                }
            }
            // Record intermediate segments too: in `pub use submodule::Item`
            // the path's res is `Item`, but `submodule` is used just as much.
            for seg in path.segments {
                if let Res::Def(_, seg_did) = seg.res
                    && let Some(local) = seg_did.as_local()
                {
                    let kind = if is_in_test_context(cx, hir_id) {
                        RefKind::TestOnly
                    } else {
                        RefKind::Real
                    };
                    let entry = self.refs.entry(local).or_insert(kind);
                    if kind == RefKind::Real {
                        *entry = RefKind::Real;
                    }
                }
            }
        }
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        if !is_candidate(&item.kind) {
            return;
        }
        if !cx
            .tcx
            .visibility(item.owner_id.def_id.to_def_id())
            .is_public()
        {
            return;
        }
        let item_name = cx.tcx.item_name(item.owner_id.def_id.to_def_id());
        if item_name == sym::main {
            return;
        }
        let attrs = cx.tcx.hir_attrs(item.hir_id());
        if attrs.iter().any(is_test_cfg_attr) {
            return;
        }
        // `#[no_mangle]` / `#[export_name]` (including the
        // `#[unsafe(no_mangle)]` form): on this nightly such attrs may not
        // appear in `hir_attrs`, so check the codegen flags the compiler
        // itself derives from them.
        let codegen_attrs = cx.tcx.codegen_fn_attrs(item.owner_id.def_id);
        if codegen_attrs
            .flags
            .intersects(CodegenFnAttrFlags::NO_MANGLE)
        {
            return;
        }
        if is_in_test_context(cx, item.hir_id()) {
            return;
        }
        self.candidates.push(item.owner_id.def_id);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let config = config_or_default::<Config>("test_only_dead_code");
        // In a binary crate nothing can be imported by another crate, so a
        // completely unreferenced item is dead regardless of
        // `ignore_unreferenced` (which only guards false positives from
        // invisible cross-crate usages in library crates).
        // `CrateType` is not nameable on this nightly; compare via Debug.
        let is_bin = cx
            .tcx
            .crate_types()
            .iter()
            .any(|t| format!("{t:?}").contains("Executable"));
        // The cross-crate false-positive caveat only applies to a library
        // crate that is actually part of a workspace: only then can another
        // member import the item. `CARGO_MANIFEST_DIR` is the manifest of the
        // (single) crate being linted in this driver process.
        let in_workspace = env::var("CARGO_MANIFEST_DIR").ok().is_some_and(|dir| {
            StdPath::new(&dir)
                .ancestors()
                .skip(1)
                .map(|p| p.join("Cargo.toml"))
                .filter_map(|p| fs::read_to_string(p).ok())
                .any(|s| {
                    s.split('\n').any(|line| {
                        // Strip an inline comment, then compare exactly; the
                        // TOML spec allows no whitespace inside the header
                        // brackets.
                        let line = line.split('#').next().unwrap_or("").trim();
                        line == "[workspace]"
                    })
                })
        });
        let cross_crate_possible = !is_bin && in_workspace;
        for local in mem::take(&mut self.candidates) {
            let kind = self.refs.get(&local);
            let dead = match kind {
                Some(RefKind::Real) => false,
                // The item is not referenced at all. This may be a false
                // positive when it is used by another crate of the workspace.
                None => !config.ignore_unreferenced || is_bin,
                Some(_) => true,
            };
            if dead {
                let maybe_cross_crate = kind.is_none() && cross_crate_possible;
                let msg = if maybe_cross_crate {
                    format!(
                        "item `{}` is never referenced; it may be used by another crate of the workspace, which is not visible here",
                        cx.tcx.item_name(local.to_def_id())
                    )
                } else {
                    format!(
                        "item `{}` is only referenced by test code",
                        cx.tcx.item_name(local.to_def_id())
                    )
                };
                // `maybe_cross_crate` implies `!config.ignore_unreferenced`
                // (such items would not be reported otherwise), so the help
                // is always the config suggestion.
                let help = if maybe_cross_crate {
                    "set `ignore_unreferenced = true` in the `[test_only_dead_code]` table of `dylint.toml` to skip such items"
                } else {
                    "delete it, or make it non-pub so rustc's dead code analysis can check it"
                };
                span_lint_and_help(
                    cx,
                    TEST_ONLY_DEAD_CODE,
                    cx.tcx.def_span(local.to_def_id()),
                    msg,
                    None,
                    help,
                );
            }
        }
    }
}
