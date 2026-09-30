//! The compiler's output. Every optional field is written out, so a consumer
//! never resolves a default.

use serde::Serialize;

pub const SCHEMA_VERSION: u32 = 1;

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RenderTree {
    pub schema: u32,
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub hash: String,
    pub compiler: String,
    pub variables: Vec<Variable>,
    pub sections: Vec<Section>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Variable {
    pub name: String,
    pub label: String,
    pub required: bool,
    pub source: VariableSource,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VariableSource {
    LabelValues { metric: String, label: String },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    pub id: String,
    pub title: Option<String>,
    pub elements: Vec<Element>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Element {
    pub id: String,
    #[serde(flatten)]
    pub kind: ElementKind,
    pub title: String,
    pub description: Option<String>,
    pub query: Query,
    pub unit: Unit,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ElementKind {
    Line {
        series: Series,
        stack: bool,
        min: Option<i64>,
        max: Option<i64>,
        thresholds: Option<Thresholds>,
    },
    Stat {
        decimals: u8,
        thresholds: Option<Thresholds>,
    },
    List {
        series: Series,
        sparkline: bool,
        thresholds: Option<Thresholds>,
    },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Query {
    pub expr: String,
    pub mode: Mode,
    pub expects: Expects,
    pub arity: Arity,
    pub uses_variables: Vec<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Range,
    Instant,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Expects {
    Matrix,
    Vector,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Arity {
    One,
    Many,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Series {
    pub label_template: Option<LabelTemplate>,
    pub reduce: Option<Reduce>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LabelTemplate {
    pub segments: Vec<Segment>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Segment {
    Label(String),
    Literal(String),
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Reduce {
    Top { n: u32, rank_by: RankBy },
    Sort { dir: SortDir, limit: u32 },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RankBy {
    Max,
    Min,
    Mean,
    Last,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDir {
    Asc,
    Desc,
    Label,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Thresholds {
    pub direction: Direction,
    pub steps: Vec<Step>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Above,
    Below,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Step {
    pub at: i64,
    pub level: Level,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Ok,
    Warning,
    Critical,
}

/// Closed enum: a consumer that does not know a value falls back to `short`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Short,
    Percent,
    Ratio,
    Count,
    Bytes,
    BytesPerSec,
    Seconds,
    Milliseconds,
    Duration,
    OpsPerSec,
    RequestsPerSec,
    Cores,
}

impl Unit {
    pub const ALL: &'static [(&'static str, Unit)] = &[
        ("short", Unit::Short),
        ("percent", Unit::Percent),
        ("ratio", Unit::Ratio),
        ("count", Unit::Count),
        ("bytes", Unit::Bytes),
        ("bytes_per_sec", Unit::BytesPerSec),
        ("seconds", Unit::Seconds),
        ("milliseconds", Unit::Milliseconds),
        ("duration", Unit::Duration),
        ("ops_per_sec", Unit::OpsPerSec),
        ("requests_per_sec", Unit::RequestsPerSec),
        ("cores", Unit::Cores),
    ];

    pub fn parse(s: &str) -> Option<Unit> {
        Self::ALL.iter().find(|(n, _)| *n == s).map(|(_, u)| *u)
    }

    /// What a reader expects to see without being told; `stat.decimals`
    /// overrides it.
    pub fn default_decimals(self) -> u8 {
        match self {
            Unit::Ratio => 2,
            Unit::Cores | Unit::Seconds => 2,
            Unit::Percent | Unit::Milliseconds => 1,
            Unit::Short
            | Unit::Count
            | Unit::Bytes
            | Unit::BytesPerSec
            | Unit::Duration
            | Unit::OpsPerSec
            | Unit::RequestsPerSec => 0,
        }
    }
}
