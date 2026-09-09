use crate::diagnostic::Diagnostic;
use serde_yaml::{Mapping, Value};

/// A mapping being read key by key. Whatever is left when `finish` runs was
/// not a field this level knows.
pub struct Fields<'a> {
    map: &'a Mapping,
    path: String,
    seen: Vec<String>,
}

impl<'a> Fields<'a> {
    pub fn new(value: &'a Value, path: &str, diags: &mut Vec<Diagnostic>) -> Option<Self> {
        match value.as_mapping() {
            Some(map) => Some(Self {
                map,
                path: path.to_string(),
                seen: Vec::new(),
            }),
            None => {
                diags.push(Diagnostic::error("E-004", path, "expected a mapping"));
                None
            }
        }
    }

    fn child(&self, key: &str) -> String {
        if self.path.is_empty() {
            key.to_string()
        } else {
            format!("{}.{key}", self.path)
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&'a Value> {
        self.seen.push(key.to_string());
        self.map
            .get(Value::String(key.to_string()))
            .filter(|v| !v.is_null())
    }

    pub fn required(&mut self, key: &str, diags: &mut Vec<Diagnostic>) -> Option<&'a Value> {
        let path = self.child(key);
        match self.get(key) {
            Some(v) => Some(v),
            None => {
                diags.push(Diagnostic::error(
                    "E-004",
                    path,
                    format!("missing required field `{key}`"),
                ));
                None
            }
        }
    }

    pub fn optional_str(&mut self, key: &str, diags: &mut Vec<Diagnostic>) -> Option<String> {
        let path = self.child(key);
        let v = self.get(key)?;
        match v.as_str() {
            Some(s) => Some(s.to_string()),
            None => {
                diags.push(Diagnostic::error("E-004", path, "expected a string"));
                None
            }
        }
    }

    pub fn required_str(&mut self, key: &str, diags: &mut Vec<Diagnostic>) -> Option<String> {
        let path = self.child(key);
        let v = self.required(key, diags)?;
        match v.as_str() {
            Some(s) => Some(s.to_string()),
            None => {
                diags.push(Diagnostic::error("E-004", path, "expected a string"));
                None
            }
        }
    }

    pub fn optional_u64(&mut self, key: &str, diags: &mut Vec<Diagnostic>) -> Option<u64> {
        let path = self.child(key);
        let v = self.get(key)?;
        match v.as_u64() {
            Some(n) => Some(n),
            None => {
                diags.push(Diagnostic::error(
                    "E-004",
                    path,
                    "expected a non-negative integer",
                ));
                None
            }
        }
    }

    pub fn required_u64(&mut self, key: &str, diags: &mut Vec<Diagnostic>) -> Option<u64> {
        let path = self.child(key);
        let v = self.required(key, diags)?;
        match v.as_u64() {
            Some(n) => Some(n),
            None => {
                diags.push(Diagnostic::error(
                    "E-004",
                    path,
                    "expected a non-negative integer",
                ));
                None
            }
        }
    }

    /// The render tree carries no floating-point, so a fractional bound is
    /// rejected rather than rounded.
    pub fn optional_i64(&mut self, key: &str, diags: &mut Vec<Diagnostic>) -> Option<i64> {
        let path = self.child(key);
        let v = self.get(key)?;
        match v.as_i64() {
            Some(n) => Some(n),
            None => {
                diags.push(Diagnostic::error("E-015", path, "expected an integer"));
                None
            }
        }
    }

    pub fn required_i64(&mut self, key: &str, diags: &mut Vec<Diagnostic>) -> Option<i64> {
        let path = self.child(key);
        let v = self.required(key, diags)?;
        match v.as_i64() {
            Some(n) => Some(n),
            None => {
                diags.push(Diagnostic::error("E-015", path, "expected an integer"));
                None
            }
        }
    }

    pub fn optional_bool(&mut self, key: &str, diags: &mut Vec<Diagnostic>) -> Option<bool> {
        let path = self.child(key);
        let v = self.get(key)?;
        match v.as_bool() {
            Some(b) => Some(b),
            None => {
                diags.push(Diagnostic::error("E-004", path, "expected a boolean"));
                None
            }
        }
    }

    /// A field belonging to a sibling variant reads as E-008.
    pub fn finish(
        self,
        diags: &mut Vec<Diagnostic>,
        misplaced_code: &'static str,
        misplaced: &[&str],
    ) {
        for (k, _) in self.map.iter() {
            let Some(key) = k.as_str() else {
                diags.push(Diagnostic::error(
                    "E-002",
                    &self.path,
                    "mapping keys must be strings",
                ));
                continue;
            };
            if self.seen.iter().any(|s| s == key) {
                continue;
            }
            let path = if self.path.is_empty() {
                key.to_string()
            } else {
                format!("{}.{key}", self.path)
            };
            if misplaced.contains(&key) {
                diags.push(Diagnostic::error(
                    misplaced_code,
                    path,
                    format!("`{key}` is not valid for this element type"),
                ));
            } else {
                diags.push(Diagnostic::error(
                    "E-002",
                    path,
                    format!("unknown field `{key}`"),
                ));
            }
        }
    }
}
