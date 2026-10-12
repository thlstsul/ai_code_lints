use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Item, ItemId, ItemKind, Node};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

rustc_lint::impl_lint_pass!(ScatteredUseStatement => [SCATTERED_USE_STATEMENT]);

#[derive(Default)]
pub struct ScatteredUseStatement {
    /// Spans of `use` statements already flagged. A single source-level `use`
    /// with nested groups (`use a::{b, c}`) is lowered to several HIR items
    /// whose spans differ but nest inside the statement's span; report each
    /// statement only once by treating overlapping spans as one statement.
    reported: Vec<Span>,
}

/// True if `span` overlaps an already-reported statement span.
fn already_reported(reported: &[Span], span: Span) -> bool {
    reported.iter().any(|s| s.contains(span) || span.contains(*s))
}

rustc_lint::declare_lint! {
    /// ### What it does
    ///
    /// Checks that `use` statements are grouped at the head of their module:
    /// a module-level `use` that appears after any non-`use` item is flagged,
    /// and `use` statements inside function bodies are flagged as well.
    ///
    /// ### Why is this bad?
    ///
    /// Scattering `use` statements throughout the module (a common pattern in
    /// AI-generated code) makes dependencies of a module hard to see at a
    /// glance and complicates refactoring.
    ///
    /// ### Known problems
    ///
    /// None.
    ///
    /// ### Example
    ///
    /// ```rust
    /// pub struct A;
    ///
    /// use std::fmt::Debug;
    /// ```
    ///
    /// Use instead:
    ///
    /// ```rust
    /// use std::fmt::Debug;
    ///
    /// pub struct A;
    /// ```
    pub SCATTERED_USE_STATEMENT,
    Warn,
    "use statement not grouped at the head of its module"
}

/// Sibling items of a module, with their spans, in source order.
fn module_children<'tcx>(cx: &LateContext<'tcx>, node: Node<'tcx>) -> Vec<(&'tcx Item<'tcx>, Span)> {
    let items: &[ItemId] = match node {
        Node::Crate(r#mod) => r#mod.item_ids,
        Node::Item(item) => match item.kind {
            ItemKind::Mod(_, r#mod) => r#mod.item_ids,
            _ => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    items
        .iter()
        .map(|&item_id| cx.tcx.hir_item(item_id))
        .map(|item| (item, item.span))
        .collect()
}

fn lint_use(cx: &LateContext<'_>, span: Span, why: &str) {
    span_lint_and_help(
        cx,
        SCATTERED_USE_STATEMENT,
        span,
        format!("{why}; move it to the head of the module"),
        None,
        "move this `use` statement to the top of the module with the other `use` statements",
    );
}

/// Items that act as imports for grouping purposes: `use`, `extern crate`,
/// and body-less `mod` declarations (`pub mod submodule;`), which only name
/// a file and are conventionally kept at the top like imports.
fn is_import_like(cx: &LateContext<'_>, kind: &ItemKind<'_>, span: Span) -> bool {
    match kind {
        ItemKind::Use(..) | ItemKind::ExternCrate(..) => true,
        // A body-less `mod foo;` declaration only names a file; distinguish
        // it from an inline `mod foo { }` by its source text.
        ItemKind::Mod(..) => cx
            .tcx
            .sess
            .source_map()
            .span_to_snippet(span)
            .is_ok_and(|s| !s.contains('{')),
        _ => false,
    }
}

impl LateLintPass<'_> for ScatteredUseStatement {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &'_ Item<'_>) {
        if !matches!(item.kind, ItemKind::Use(..)) {
            return;
        }

        // One flag per source-level `use` statement: nested use trees are
        // lowered to several HIR items whose spans nest inside the
        // statement's span.
        if already_reported(&self.reported, item.span) {
            return;
        }
        self.reported.push(item.span);

        // Module-level `use` items have a `Crate` or `Mod` parent node; a
        // `use` inside a body (e.g. a function) has some other node in
        // between (its enclosing block/owner).
        let module_level = matches!(
            cx.tcx.hir_parent_iter(item.hir_id()).next().map(|(_, node)| node),
            Some(Node::Crate(_))
        ) || matches!(
            cx.tcx.hir_parent_iter(item.hir_id()).next().map(|(_, node)| node),
            Some(Node::Item(parent)) if matches!(parent.kind, ItemKind::Mod(..))
        );
        if !module_level {
            lint_use(cx, item.span, "use statement inside a body");
            return;
        }

        // Check ordering among the module's items.
        let parent = match cx.tcx.hir_parent_iter(item.hir_id()).next() {
            Some((_, node @ (Node::Crate(_) | Node::Item(_)))) => node,
            _ => return,
        };
        let mut seen_non_use = false;
        for (sibling, span) in module_children(cx, parent) {
            if sibling.hir_id() == item.hir_id() {
                if seen_non_use {
                    lint_use(cx, span, "use statement after a non-use item");
                }
                return;
            }
            if !is_import_like(cx, &sibling.kind, span) {
                seen_non_use = true;
            }
        }
    }
}
