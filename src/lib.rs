//! Compiles the Vedavid dashboard DSL into a render tree, with no I/O.

pub mod diagnostic;
pub mod dsl;
pub mod tree;

mod compile;

pub use compile::canonical_json;
pub use diagnostic::{Diagnostic, Severity};
pub use tree::RenderTree;

/// Compiles a document, discarding warnings. See
/// [`compile_with_diagnostics`] when warnings matter, as they do for `lint`.
pub fn compile(yaml: &str) -> Result<RenderTree, Vec<Diagnostic>> {
    let (tree, diags) = compile_with_diagnostics(yaml);
    match tree {
        Some(tree) => Ok(tree),
        None => Err(diags),
    }
}

/// Every diagnostic for the document, alongside the tree when it compiled.
pub fn compile_with_diagnostics(yaml: &str) -> (Option<RenderTree>, Vec<Diagnostic>) {
    let (document, mut diags) = dsl::parse(yaml);
    let Some(document) = document else {
        return (None, diags);
    };
    let (tree, more) = compile::lower(document);
    diags.extend(more);
    if diags.iter().any(Diagnostic::is_error) {
        (None, diags)
    } else {
        (Some(tree), diags)
    }
}

/// Pretty-printed JSON with sorted keys, which is what the CLI writes and
/// what the conformance corpus stores.
pub fn to_json(tree: &RenderTree) -> String {
    let value = serde_json::to_value(tree).expect("the render tree serializes");
    serde_json::to_string_pretty(&value).expect("a JSON value serializes")
}
