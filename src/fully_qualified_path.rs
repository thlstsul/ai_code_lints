use std::collections::HashMap;
use std::mem;

use clippy_utils::diagnostics::span_lint_and_help;
use dylint_linting::config_or_default;
use rustc_hir::def::Res;
use rustc_hir::{HirId, ItemKind, Node, Path};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, SyntaxContext, kw};

rustc_lint::declare_lint! {
    /// ### What it does
    ///
    /// Checks for fully qualified paths that are used inline instead of being
    /// imported with a `use` statement: paths starting with an external crate
    /// name (`std::fmt::Debug`) or with `crate::` and at least one further
    /// segment (`crate::util::helper`).
    ///
    /// ### Why is this bad?
    ///
    /// AI-generated code tends to spell out full paths at every use site,
    /// which makes call sites noisy and hides which dependencies a module
    /// actually has. Importing the used items at the module head is the
    /// idiomatic style.
    ///
    /// ### Known problems
    ///
    /// Paths written only once (e.g. a single `std::env::args()` in `main`)
    /// are flagged by default. Set `min_occurrences` in the
    /// `[fully_qualified_path_without_use]` table of the workspace's
    /// `dylint.toml` (e.g. `min_occurrences = 2`) to flag a crate's fully
    /// qualified paths only when they occur at least that many times.
    ///
    /// ### Example
    ///
    /// ```rust
    /// pub fn f() {
    ///     std::mem::drop(0);
    /// }
    /// ```
    ///
    /// Use instead:
    ///
    /// ```rust
    /// use std::mem::drop;
    ///
    /// pub fn f() {
    ///     drop(0);
    /// }
    /// ```
    pub FULLY_QUALIFIED_PATH_WITHOUT_USE,
    Warn,
    "fully qualified path used instead of a `use` import"
}

rustc_lint::impl_lint_pass!(FullyQualifiedPathWithoutUse => [FULLY_QUALIFIED_PATH_WITHOUT_USE]);

#[derive(Default)]
pub struct FullyQualifiedPathWithoutUse {
    /// Number of flagged paths per first-segment crate name, deferred to
    /// `check_crate_post` so a configurable minimum can be applied.
    occurrences: HashMap<String, Vec<(Span, String)>>,
}

/// Configuration for `FULLY_QUALIFIED_PATH_WITHOUT_USE`, read from the
/// workspace's `dylint.toml` (`[fully_qualified_path_without_use]` table).
#[derive(serde::Deserialize)]
#[serde(default)]
struct Config {
    /// Only flag a crate's fully qualified paths when they occur at least
    /// this many times; single occurrences (e.g. one `std::env::args()` in
    /// `main`) stay unflagged when set above 1.
    min_occurrences: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self { min_occurrences: 1 }
    }
}

impl<'tcx> LateLintPass<'tcx> for FullyQualifiedPathWithoutUse {
    fn check_path(&mut self, cx: &LateContext<'tcx>, path: &Path<'tcx>, hir_id: HirId) {
        // Skip anything not written directly in source: derive/proc-macro
        // expansions (e.g. `#[derive(Parser)]` generating struct fields) have
        // spans that pass `from_expansion()` checks for call-site tokens but
        // whose syntax context belongs to the macro.
        if path.span.ctxt() != SyntaxContext::root() {
            return;
        }
        if path.span.from_expansion() || path.segments.len() < 2 {
            return;
        }
        // Skip paths that are part of a `use` or `extern crate` item. For a
        // use item's own path, `hir_id` IS the item's id (so check the node
        // itself); for paths deeper in a use tree, check the ancestor chain.
        let in_use_item = |node: Node| {
            matches!(
                node,
                Node::Item(item) if matches!(item.kind, ItemKind::Use(..) | ItemKind::ExternCrate(..))
            )
        };
        // Skip type alias definitions (`pub type Result<T> = std::result::Result<T, Error>;`):
        // the path is the alias's definition, not a use site, and importing the
        // aliased path instead would collide with the alias's own name.
        let in_ty_alias = |node: Node| {
            matches!(
                node,
                Node::Item(item) if matches!(item.kind, ItemKind::TyAlias(..))
            )
        };
        if in_use_item(cx.tcx.hir_node(hir_id))
            || in_ty_alias(cx.tcx.hir_node(hir_id))
            || cx
                .tcx
                .hir_parent_iter(hir_id)
                .any(|(_, node)| in_use_item(node) || in_ty_alias(node))
        {
            return;
        }

        let first = &path.segments[0];
        let starts_at_crate_root = match first.res {
            Res::Def(_, did) => did.is_crate_root(),
            _ => false,
        };
        let starts_at_crate_kw = first.ident.name == kw::Crate;
        if !starts_at_crate_root && !starts_at_crate_kw {
            return;
        }

        let snippet = match cx.tcx.sess.source_map().span_to_snippet(path.span) {
            Ok(s) => s,
            Err(_) => return,
        };
        // Derive macros (e.g. clap) may rewrite a user-written short type
        // token into a multi-segment path while keeping the original token's
        // span, so `from_expansion()` and the syntax-context check above do
        // not fire. If the snippet does not spell out the HIR path's leading
        // segments, the path semantics were not written by the user.
        // (Whitespace is stripped first so multi-argument generics such as
        // `std::io::Result<(A, B)>` are not rejected.)
        let path_prefix = path
            .segments
            .iter()
            .map(|s| s.ident.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let snippet: String = snippet.chars().filter(|c| !c.is_whitespace()).collect();
        if !snippet.starts_with(&path_prefix) {
            return;
        }
        let crate_name = if starts_at_crate_kw {
            "crate".to_owned()
        } else {
            first.ident.name.to_string()
        };
        self.occurrences
            .entry(crate_name)
            .or_default()
            .push((path.span, snippet));
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let config = config_or_default::<Config>("fully_qualified_path_without_use");
        let occurrences = mem::take(&mut self.occurrences);
        // 遍历按键排序：HashMap 迭代顺序不定，会导致同一份代码两次运行的
        // 告警顺序不同（UI 测试因此不稳定）。
        let mut crate_names: Vec<_> = occurrences.keys().cloned().collect();
        crate_names.sort_unstable();
        for crate_name in crate_names {
            let spans = &occurrences[&crate_name];
            if spans.len() < config.min_occurrences {
                continue;
            }
            for (span, snippet) in spans.iter() {
                let help = if crate_name == "crate" {
                    let item = snippet
                        .rsplit("::")
                        .next()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(snippet.as_str());
                    format!("import `{item}` with a `use` statement at the module head")
                } else {
                    format!(
                        "import `{crate_name}` (or its used item) with a `use` statement at the module head"
                    )
                };
                span_lint_and_help(
                    cx,
                    FULLY_QUALIFIED_PATH_WITHOUT_USE,
                    *span,
                    format!("fully qualified path `{snippet}` used instead of a `use` import"),
                    None,
                    help,
                );
            }
        }
    }
}
