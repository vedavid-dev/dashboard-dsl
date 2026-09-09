//! Deserialization and validation of the input document.

mod fields;
mod template;

pub use template::parse_template;

use crate::diagnostic::Diagnostic;
use crate::tree::{Direction, Level, RankBy, SortDir, Step, Thresholds, Unit};
use fields::Fields;
use serde_yaml::Value;

pub const SUPPORTED_VERSION: u64 = 1;

const MAX_TITLE: usize = 60;
const MAX_DESCRIPTION: usize = 200;
const MAX_ELEMENTS_PER_SECTION: usize = 24;
pub const MAX_TOP: u64 = 20;
pub const MAX_LIMIT: u64 = 100;
pub const DEFAULT_LIMIT: u64 = 20;

#[derive(Debug)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub variables: Vec<Variable>,
    pub sections: Vec<Section>,
}

#[derive(Debug)]
pub struct Variable {
    pub name: String,
    pub label: String,
    pub metric: String,
    pub label_name: String,
}

#[derive(Debug)]
pub struct Section {
    pub title: Option<String>,
    pub elements: Vec<Element>,
    /// Where the author wrote these elements, so a diagnostic locates the source.
    pub path: String,
}

#[derive(Debug)]
pub struct Element {
    pub id: Option<String>,
    pub title: String,
    pub query: String,
    pub unit: Unit,
    pub description: Option<String>,
    pub kind: Kind,
}

#[derive(Debug)]
pub enum Kind {
    Line {
        label: Option<String>,
        top: Option<u64>,
        rank_by: RankBy,
        stack: bool,
        min: Option<i64>,
        max: Option<i64>,
    },
    Stat {
        decimals: u8,
        thresholds: Option<Thresholds>,
    },
    List {
        label: String,
        sort: SortDir,
        limit: u64,
        sparkline: bool,
        thresholds: Option<Thresholds>,
    },
}

impl Kind {
    pub fn type_name(&self) -> &'static str {
        match self {
            Kind::Line { .. } => "line",
            Kind::Stat { .. } => "stat",
            Kind::List { .. } => "list",
        }
    }
}

/// Every field name any element type accepts, so a misplaced one reads as E-008.
const ELEMENT_FIELDS: &[&str] = &[
    "type",
    "title",
    "query",
    "unit",
    "id",
    "description",
    "series",
    "stack",
    "min",
    "max",
    "thresholds",
    "decimals",
    "label",
    "sort",
    "limit",
    "sparkline",
];

pub fn parse(yaml: &str) -> (Option<Document>, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let value: Value = match serde_yaml::from_str(yaml) {
        Ok(v) => v,
        Err(e) => {
            let mut d = Diagnostic::error("E-001", "", format!("YAML is not well-formed: {e}"));
            if let Some(loc) = e.location() {
                d = d.at(loc.line(), loc.column());
            }
            diags.push(d);
            return (None, diags);
        }
    };

    let Some(mut root) = Fields::new(&value, "", &mut diags) else {
        return (None, diags);
    };

    match root.required_u64("version", &mut diags) {
        Some(v) if v == SUPPORTED_VERSION => {}
        Some(v) => diags.push(Diagnostic::error(
            "E-003",
            "version",
            format!("unsupported version {v}; this compiler supports version {SUPPORTED_VERSION}"),
        )),
        None => {}
    }

    let id = root
        .required_str("id", &mut diags)
        .and_then(|s| check_pattern(&s, "id", is_dashboard_id, &mut diags));
    let title = root
        .required_str("title", &mut diags)
        .and_then(|s| check_len(&s, "title", 1, MAX_TITLE, &mut diags));

    let variables = match root.get("variables") {
        Some(v) => parse_variables(v, &mut diags),
        None => Vec::new(),
    };

    let has_sections = root.get("sections").is_some();
    let has_elements = root.get("elements").is_some();
    let sections = match (has_sections, has_elements) {
        (true, true) => {
            diags.push(Diagnostic::error(
                "E-012",
                "",
                "`sections` and `elements` are mutually exclusive",
            ));
            Vec::new()
        }
        (false, false) => {
            diags.push(Diagnostic::error(
                "E-012",
                "",
                "a document must declare either `sections` or `elements`",
            ));
            Vec::new()
        }
        // The shorthand becomes one untitled section, so consumers see one shape.
        (false, true) => {
            let v = root.get("elements").unwrap().clone();
            let elements = parse_elements(&v, "elements", &mut diags);
            vec![Section {
                title: None,
                elements,
                path: "elements".to_string(),
            }]
        }
        (true, false) => {
            let v = root.get("sections").unwrap().clone();
            parse_sections(&v, &mut diags)
        }
    };

    root.finish(&mut diags, "E-002", &[]);

    match (id, title) {
        (Some(id), Some(title)) => (
            Some(Document {
                id,
                title,
                variables,
                sections,
            }),
            diags,
        ),
        _ => (None, diags),
    }
}

fn parse_variables(value: &Value, diags: &mut Vec<Diagnostic>) -> Vec<Variable> {
    let Some(items) = as_sequence(value, "variables", diags) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let path = format!("variables[{i}]");
        let Some(mut f) = Fields::new(item, &path, diags) else {
            continue;
        };
        let name = f
            .required_str("name", diags)
            .and_then(|s| check_pattern(&s, &format!("{path}.name"), is_variable_name, diags));
        let label = f.optional_str("label", diags);
        let source = f.required("source", diags).cloned();
        f.finish(diags, "E-002", &[]);

        let (metric, label_name) = match source {
            Some(s) => {
                let sp = format!("{path}.source");
                match Fields::new(&s, &sp, diags) {
                    Some(mut sf) => {
                        let m = sf.required_str("metric", diags);
                        let l = sf.required_str("label", diags);
                        sf.finish(diags, "E-002", &[]);
                        (m, l)
                    }
                    None => (None, None),
                }
            }
            None => (None, None),
        };

        if let (Some(name), Some(metric), Some(label_name)) = (name, metric, label_name) {
            let label = label.unwrap_or_else(|| titlecase(&name));
            out.push(Variable {
                name,
                label,
                metric,
                label_name,
            });
        }
    }

    let mut seen: Vec<&str> = Vec::new();
    for v in &out {
        if seen.contains(&v.name.as_str()) {
            diags.push(Diagnostic::error(
                "E-011",
                "variables",
                format!("duplicate variable name `{}`", v.name),
            ));
        }
        seen.push(&v.name);
    }
    out
}

fn parse_sections(value: &Value, diags: &mut Vec<Diagnostic>) -> Vec<Section> {
    let Some(items) = as_sequence(value, "sections", diags) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let path = format!("sections[{i}]");
        let Some(mut f) = Fields::new(item, &path, diags) else {
            continue;
        };
        let title = f
            .optional_str("title", diags)
            .and_then(|s| check_len(&s, &format!("{path}.title"), 1, MAX_TITLE, diags));
        let elements_path = format!("{path}.elements");
        let elements = match f.required("elements", diags).cloned() {
            Some(v) => parse_elements(&v, &elements_path, diags),
            None => Vec::new(),
        };
        f.finish(diags, "E-002", &[]);
        if elements.is_empty() {
            diags.push(Diagnostic::error(
                "E-015",
                format!("{path}.elements"),
                "a section must contain at least one element",
            ));
        } else if elements.len() > MAX_ELEMENTS_PER_SECTION {
            diags.push(Diagnostic::error(
                "E-015",
                format!("{path}.elements"),
                format!(
                    "{} elements exceeds the maximum of {MAX_ELEMENTS_PER_SECTION}",
                    elements.len()
                ),
            ));
        }
        out.push(Section {
            title,
            elements,
            path: elements_path,
        });
    }

    let mut seen: Vec<&str> = Vec::new();
    for s in &out {
        if let Some(t) = &s.title {
            if seen.contains(&t.as_str()) {
                diags.push(Diagnostic::error(
                    "E-011",
                    "sections",
                    format!("duplicate section title `{t}`"),
                ));
            }
            seen.push(t);
        }
    }
    out
}

fn parse_elements(value: &Value, path: &str, diags: &mut Vec<Diagnostic>) -> Vec<Element> {
    let Some(items) = as_sequence(value, path, diags) else {
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| parse_element(item, &format!("{path}[{i}]"), diags))
        .collect()
}

fn parse_element(value: &Value, path: &str, diags: &mut Vec<Diagnostic>) -> Option<Element> {
    let mut f = Fields::new(value, path, diags)?;
    let ty = f.required_str("type", diags)?;
    let title = f
        .required_str("title", diags)
        .and_then(|s| check_len(&s, &format!("{path}.title"), 1, MAX_TITLE, diags));
    let query = f.required_str("query", diags);
    let unit = match f.required_str("unit", diags) {
        Some(u) => match Unit::parse(&u) {
            Some(unit) => Some(unit),
            None => {
                diags.push(Diagnostic::error(
                    "E-007",
                    format!("{path}.unit"),
                    format!("unknown unit `{u}`"),
                ));
                None
            }
        },
        None => None,
    };
    let id = f.optional_str("id", diags);
    let description = f.optional_str("description", diags).and_then(|s| {
        check_len(
            &s,
            &format!("{path}.description"),
            0,
            MAX_DESCRIPTION,
            diags,
        )
    });

    let kind = match ty.as_str() {
        "line" => parse_line(&mut f, path, diags),
        "stat" => parse_stat(&mut f, path, unit, diags),
        "list" => parse_list(&mut f, path, diags),
        other => {
            diags.push(Diagnostic::error(
                "E-006",
                format!("{path}.type"),
                format!("unknown element type `{other}`"),
            ));
            None
        }
    };

    f.finish(diags, "E-008", ELEMENT_FIELDS);

    Some(Element {
        id,
        title: title?,
        query: query?,
        unit: unit?,
        description,
        kind: kind?,
    })
}

fn parse_line(f: &mut Fields, path: &str, diags: &mut Vec<Diagnostic>) -> Option<Kind> {
    let mut label = None;
    let mut top = None;
    let mut rank_by = RankBy::Max;
    let mut rank_by_given = false;

    if let Some(series) = f.get("series").cloned() {
        let sp = format!("{path}.series");
        if let Some(mut sf) = Fields::new(&series, &sp, diags) {
            label = sf.optional_str("label", diags);
            top = sf
                .optional_u64("top", diags)
                .and_then(|n| check_range(n, &format!("{sp}.top"), 1, MAX_TOP, diags));
            if let Some(r) = sf.optional_str("rank_by", diags) {
                rank_by_given = true;
                rank_by = match r.as_str() {
                    "max" => RankBy::Max,
                    "min" => RankBy::Min,
                    "mean" => RankBy::Mean,
                    "last" => RankBy::Last,
                    other => {
                        diags.push(Diagnostic::error(
                            "E-009",
                            format!("{sp}.rank_by"),
                            format!("unknown rank_by `{other}`"),
                        ));
                        RankBy::Max
                    }
                };
            }
            sf.finish(diags, "E-002", &[]);
        }
    }

    if rank_by_given && top.is_none() {
        diags.push(Diagnostic::error(
            "E-009",
            format!("{path}.series.rank_by"),
            "`rank_by` requires `top`",
        ));
    }

    Some(Kind::Line {
        label,
        top,
        rank_by,
        stack: f.optional_bool("stack", diags).unwrap_or(false),
        min: f.optional_i64("min", diags),
        max: f.optional_i64("max", diags),
    })
}

fn parse_stat(
    f: &mut Fields,
    path: &str,
    unit: Option<Unit>,
    diags: &mut Vec<Diagnostic>,
) -> Option<Kind> {
    let decimals = match f.optional_u64("decimals", diags) {
        Some(d) => check_range(d, &format!("{path}.decimals"), 0, 4, diags).map(|d| d as u8),
        None => Some(unit.map(Unit::default_decimals).unwrap_or(0)),
    }?;
    Some(Kind::Stat {
        decimals,
        thresholds: parse_thresholds(f, path, diags),
    })
}

fn parse_list(f: &mut Fields, path: &str, diags: &mut Vec<Diagnostic>) -> Option<Kind> {
    let label = f.required_str("label", diags);
    let sort = match f.optional_str("sort", diags) {
        Some(s) => match s.as_str() {
            "asc" => SortDir::Asc,
            "desc" => SortDir::Desc,
            "label" => SortDir::Label,
            other => {
                diags.push(Diagnostic::error(
                    "E-009",
                    format!("{path}.sort"),
                    format!("unknown sort `{other}`"),
                ));
                SortDir::Desc
            }
        },
        None => SortDir::Desc,
    };
    let limit = match f.optional_u64("limit", diags) {
        Some(l) => check_range(l, &format!("{path}.limit"), 1, MAX_LIMIT, diags)?,
        None => DEFAULT_LIMIT,
    };
    Some(Kind::List {
        label: label?,
        sort,
        limit,
        sparkline: f.optional_bool("sparkline", diags).unwrap_or(false),
        thresholds: parse_thresholds(f, path, diags),
    })
}

fn parse_thresholds(f: &mut Fields, path: &str, diags: &mut Vec<Diagnostic>) -> Option<Thresholds> {
    let value = f.get("thresholds").cloned()?;
    let tp = format!("{path}.thresholds");
    let mut tf = Fields::new(&value, &tp, diags)?;
    let direction = match tf.required_str("direction", diags)?.as_str() {
        "above" => Direction::Above,
        "below" => Direction::Below,
        other => {
            diags.push(Diagnostic::error(
                "E-009",
                format!("{tp}.direction"),
                format!("unknown direction `{other}`"),
            ));
            return None;
        }
    };
    let steps_value = tf.required("steps", diags).cloned();
    tf.finish(diags, "E-002", &[]);

    let steps_value = steps_value?;
    let items = as_sequence(&steps_value, &format!("{tp}.steps"), diags)?;
    let mut steps = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let sp = format!("{tp}.steps[{i}]");
        let Some(mut sf) = Fields::new(item, &sp, diags) else {
            continue;
        };
        let at = sf.required_i64("at", diags);
        let level = match sf.required_str("level", diags) {
            Some(l) => match l.as_str() {
                "ok" => Some(Level::Ok),
                "warning" => Some(Level::Warning),
                "critical" => Some(Level::Critical),
                other => {
                    diags.push(Diagnostic::error(
                        "E-009",
                        format!("{sp}.level"),
                        format!("unknown level `{other}`"),
                    ));
                    None
                }
            },
            None => None,
        };
        sf.finish(diags, "E-002", &[]);
        if let (Some(at), Some(level)) = (at, level) {
            steps.push(Step { at, level });
        }
    }

    if steps.is_empty() {
        diags.push(Diagnostic::error(
            "E-015",
            format!("{tp}.steps"),
            "thresholds must declare at least one step",
        ));
        return None;
    }

    // An out-of-order list usually means the author meant something else.
    let ordered = match direction {
        Direction::Above => steps.windows(2).all(|w| w[0].at < w[1].at),
        Direction::Below => steps.windows(2).all(|w| w[0].at > w[1].at),
    };
    if !ordered {
        let (order, dir) = match direction {
            Direction::Above => ("ascending", "above"),
            Direction::Below => ("descending", "below"),
        };
        diags.push(Diagnostic::error(
            "E-014",
            format!("{tp}.steps"),
            format!("steps must be sorted {order} by `at` for direction `{dir}`"),
        ));
        return None;
    }

    Some(Thresholds { direction, steps })
}

fn as_sequence<'a>(
    value: &'a Value,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<&'a Vec<Value>> {
    match value.as_sequence() {
        Some(s) => Some(s),
        None => {
            diags.push(Diagnostic::error("E-004", path, "expected a list"));
            None
        }
    }
}

fn check_pattern(
    s: &str,
    path: &str,
    ok: fn(&str) -> bool,
    diags: &mut Vec<Diagnostic>,
) -> Option<String> {
    if ok(s) {
        Some(s.to_string())
    } else {
        diags.push(Diagnostic::error(
            "E-005",
            path,
            format!("`{s}` does not match the required pattern"),
        ));
        None
    }
}

fn check_len(
    s: &str,
    path: &str,
    min: usize,
    max: usize,
    diags: &mut Vec<Diagnostic>,
) -> Option<String> {
    let n = s.chars().count();
    if n >= min && n <= max {
        Some(s.to_string())
    } else {
        diags.push(Diagnostic::error(
            "E-015",
            path,
            format!("length {n} is outside the permitted range {min}..={max}"),
        ));
        None
    }
}

fn check_range(n: u64, path: &str, min: u64, max: u64, diags: &mut Vec<Diagnostic>) -> Option<u64> {
    if n >= min && n <= max {
        Some(n)
    } else {
        diags.push(Diagnostic::error(
            "E-015",
            path,
            format!("{n} is outside the permitted range {min}..={max}"),
        ));
        None
    }
}

fn is_dashboard_id(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > 64 {
        return false;
    }
    let alnum = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    alnum(b[0]) && alnum(b[b.len() - 1]) && b.iter().all(|&c| alnum(c) || c == b'-')
}

fn is_variable_name(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > 32 {
        return false;
    }
    b[0].is_ascii_lowercase()
        && b.iter()
            .all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

fn titlecase(name: &str) -> String {
    let spaced = name.replace('_', " ");
    let mut c = spaced.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => spaced,
    }
}
