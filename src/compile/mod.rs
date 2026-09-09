//! The transform: derive, resolve identity, and hash.

use crate::diagnostic::Diagnostic;
use crate::dsl::{self, Document, Kind};
use crate::tree::*;
use sha2::{Digest, Sha256};

const MAX_ELEMENTS_BEFORE_WARNING: usize = 12;

pub fn lower(doc: Document) -> (RenderTree, Vec<Diagnostic>) {
    let mut diags = Vec::new();

    let variables: Vec<Variable> = doc
        .variables
        .iter()
        .map(|v| Variable {
            name: v.name.clone(),
            label: v.label.clone(),
            required: true,
            source: VariableSource::LabelValues {
                metric: v.metric.clone(),
                label: v.label_name.clone(),
            },
        })
        .collect();

    let declared: Vec<&str> = doc.variables.iter().map(|v| v.name.as_str()).collect();
    let mut referenced: Vec<String> = Vec::new();
    let mut element_ids: Vec<String> = Vec::new();
    let mut index = 0usize;
    let mut sections = Vec::new();

    for (s, section) in doc.sections.iter().enumerate() {
        let mut elements = Vec::new();
        for element in &section.elements {
            let path = format!("{}[{}]", section.path, elements.len());
            let uses = variables_in(&element.query);
            for name in &uses {
                if !declared.contains(&name.as_str()) {
                    diags.push(Diagnostic::error(
                        "E-010",
                        format!("{path}.query"),
                        format!("query references undeclared variable ${name}"),
                    ));
                }
                if !referenced.contains(name) {
                    referenced.push(name.clone());
                }
            }

            let id = element
                .id
                .clone()
                .unwrap_or_else(|| format!("{}.e{index}", doc.id));
            if element_ids.contains(&id) {
                diags.push(Diagnostic::error(
                    "E-011",
                    format!("{path}.id"),
                    format!("duplicate element id `{id}`"),
                ));
            }
            element_ids.push(id.clone());
            index += 1;

            let (mode, expects, arity) = shape(&element.kind);
            let kind = match &element.kind {
                Kind::Line {
                    label,
                    top,
                    rank_by,
                    stack,
                    min,
                    max,
                } => {
                    let label_template = label.as_ref().and_then(|l| {
                        dsl::parse_template(l, &format!("{path}.series.label"), &mut diags)
                    });
                    ElementKind::Line {
                        series: Series {
                            label_template,
                            reduce: top.map(|n| Reduce::Top {
                                n: n as u32,
                                rank_by: *rank_by,
                            }),
                        },
                        stack: *stack,
                        min: *min,
                        max: *max,
                        thresholds: None,
                    }
                }
                Kind::Stat {
                    decimals,
                    thresholds,
                } => {
                    // A single series cannot be proven without running the query.
                    if element.query.contains("by (") || element.query.contains("by(") {
                        diags.push(Diagnostic::warning(
                            "W-004",
                            format!("{path}.query"),
                            "a `stat` query with a `by (` clause is likely to return more than one series",
                        ));
                    }
                    ElementKind::Stat {
                        decimals: *decimals,
                        thresholds: thresholds.clone(),
                    }
                }
                Kind::List {
                    label,
                    sort,
                    limit,
                    sparkline,
                    thresholds,
                } => {
                    if !*sparkline && *limit == dsl::MAX_LIMIT {
                        diags.push(Diagnostic::warning(
                            "W-001",
                            format!("{path}.limit"),
                            format!("a list of {limit} rows without a sparkline is unlikely to be readable on a phone"),
                        ));
                    }
                    ElementKind::List {
                        series: Series {
                            label_template: dsl::parse_template(
                                label,
                                &format!("{path}.label"),
                                &mut diags,
                            ),
                            reduce: Some(Reduce::Sort {
                                dir: *sort,
                                limit: *limit as u32,
                            }),
                        },
                        sparkline: *sparkline,
                        thresholds: thresholds.clone(),
                    }
                }
            };

            elements.push(Element {
                id,
                kind,
                title: element.title.clone(),
                description: element.description.clone(),
                query: Query {
                    expr: element.query.clone(),
                    mode,
                    expects,
                    arity,
                    uses_variables: uses,
                },
                unit: element.unit,
            });
        }
        sections.push(Section {
            id: format!("{}.s{s}", doc.id),
            title: section.title.clone(),
            elements,
        });
    }

    for v in &doc.variables {
        if !referenced.contains(&v.name) {
            diags.push(Diagnostic::warning(
                "W-002",
                "variables",
                format!("variable `{}` is declared but never referenced", v.name),
            ));
        }
    }
    if index > MAX_ELEMENTS_BEFORE_WARNING {
        diags.push(Diagnostic::warning(
            "W-003",
            "",
            format!("{index} elements makes for a long scroll on a phone"),
        ));
    }

    let mut tree = RenderTree {
        schema: SCHEMA_VERSION,
        id: doc.id,
        title: doc.title,
        hash: String::new(),
        compiler: env!("CARGO_PKG_VERSION").to_string(),
        variables,
        sections,
    };
    tree.hash = hash(&tree);
    (tree, diags)
}

fn shape(kind: &Kind) -> (Mode, Expects, Arity) {
    match kind {
        Kind::Line { .. } => (Mode::Range, Expects::Matrix, Arity::Many),
        Kind::Stat { .. } => (Mode::Instant, Expects::Vector, Arity::One),
        Kind::List { .. } => (Mode::Instant, Expects::Vector, Arity::Many),
    }
}

/// A lexical scan, so a consumer needs no PromQL parser to read this.
fn variables_in(query: &str) -> Vec<String> {
    let bytes = query.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            if end > start && (bytes[start].is_ascii_alphabetic() || bytes[start] == b'_') {
                let name = query[start..end].to_string();
                if !out.contains(&name) {
                    out.push(name);
                }
                i = end;
                continue;
            }
        }
        i += 1;
    }
    out.sort();
    out
}

/// Canonical JSON with sorted keys, excluding the fields that are not part of
/// the dashboard's meaning.
pub fn canonical_json(tree: &RenderTree) -> String {
    let mut value = serde_json::to_value(tree).expect("the render tree serializes");
    if let Some(map) = value.as_object_mut() {
        map.remove("hash");
        map.remove("compiler");
    }
    serde_json::to_string(&value).expect("a JSON value serializes")
}

fn hash(tree: &RenderTree) -> String {
    let digest = Sha256::digest(canonical_json(tree).as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    hex[..16].to_string()
}
