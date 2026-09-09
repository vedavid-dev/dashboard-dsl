use crate::diagnostic::Diagnostic;
use crate::tree::{LabelTemplate, Segment};

/// Literal text interleaved with `{{label_name}}`, parsed at compile time.
pub fn parse_template(
    input: &str,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<LabelTemplate> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i..].starts_with(b"{{") {
            let Some(end) = input[i + 2..].find("}}") else {
                diags.push(Diagnostic::error(
                    "E-013",
                    path,
                    "unclosed `{{` in label template",
                ));
                return None;
            };
            let name = &input[i + 2..i + 2 + end];
            if !is_label_name(name) {
                diags.push(Diagnostic::error(
                    "E-013",
                    path,
                    format!("`{name}` is not a valid label name"),
                ));
                return None;
            }
            if !literal.is_empty() {
                segments.push(Segment::Literal(std::mem::take(&mut literal)));
            }
            segments.push(Segment::Label(name.to_string()));
            i += 2 + end + 2;
        } else if bytes[i..].starts_with(b"}}") {
            diags.push(Diagnostic::error(
                "E-013",
                path,
                "`}}` without a matching `{{` in label template",
            ));
            return None;
        } else {
            literal.push(input[i..].chars().next().unwrap());
            i += input[i..].chars().next().unwrap().len_utf8();
        }
    }

    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    if segments.is_empty() {
        diags.push(Diagnostic::error("E-013", path, "label template is empty"));
        return None;
    }
    Some(LabelTemplate { segments })
}

fn is_label_name(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && (b[0].is_ascii_alphabetic() || b[0] == b'_')
        && b.iter().all(|&c| c.is_ascii_alphanumeric() || c == b'_')
}
