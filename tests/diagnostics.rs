//! One test per diagnostic code. The corpus covers whole documents; this
//! covers the codes the corpus does not reach.

use vedavid_dashboard_dsl::{compile_with_diagnostics, Diagnostic};

fn codes(yaml: &str) -> Vec<&'static str> {
    let (_, diags) = compile_with_diagnostics(yaml);
    diags.iter().map(|d| d.code).collect()
}

fn diags(yaml: &str) -> Vec<Diagnostic> {
    compile_with_diagnostics(yaml).1
}

const STAT: &str = "
    - type: stat
      title: Pods
      query: count(kube_pod_info)
      unit: count";

fn doc(body: &str) -> String {
    format!("version: 1\nid: d\ntitle: T\nelements:{body}\n")
}

#[test]
fn e001_malformed_yaml() {
    let d = diags("version: 1\n  id: [unclosed\n");
    assert_eq!(d[0].code, "E-001");
    assert!(d[0].line.is_some(), "a parse error carries a location");
}

#[test]
fn e002_unknown_field_at_the_top_level() {
    assert!(codes(&doc(STAT).replace("title: T", "title: T\nauthor: me")).contains(&"E-002"));
}

#[test]
fn e003_unsupported_version() {
    assert!(codes(&doc(STAT).replace("version: 1", "version: 2")).contains(&"E-003"));
}

#[test]
fn e004_missing_required_field() {
    assert!(codes("version: 1\nid: d\nelements: []\n").contains(&"E-004"));
}

#[test]
fn e005_bad_id_and_variable_name() {
    assert!(codes(&doc(STAT).replace("id: d", "id: Not_An_Id")).contains(&"E-005"));
    let yaml = "version: 1\nid: d\ntitle: T\nvariables:\n  - name: Bad-Name\n    source:\n      metric: m\n      label: l\nelements:\n  - type: stat\n    title: T\n    query: q\n    unit: count\n";
    assert!(codes(yaml).contains(&"E-005"));
}

#[test]
fn e006_unknown_element_type() {
    assert!(codes(&doc(STAT).replace("type: stat", "type: heatmap")).contains(&"E-006"));
}

#[test]
fn e007_unknown_unit() {
    assert!(codes(&doc(STAT).replace("unit: count", "unit: furlongs")).contains(&"E-007"));
}

#[test]
fn e008_field_belonging_to_another_element_type() {
    let yaml = doc("
    - type: line
      title: CPU
      query: q
      unit: cores
      sparkline: true");
    let d = diags(&yaml);
    assert!(d.iter().any(|x| x.code == "E-008"), "{d:?}");
    assert!(d.iter().any(|x| x.message.contains("sparkline")));
}

#[test]
fn e009_dependent_field_violation() {
    let yaml = doc("
    - type: list
      title: Pods
      query: q
      unit: bytes
      label: \"{{pod}}\"
      sort: sideways");
    assert!(codes(&yaml).contains(&"E-009"));
}

#[test]
fn e011_duplicate_ids_and_names() {
    let yaml = doc("
    - type: stat
      id: same
      title: A
      query: q
      unit: count
    - type: stat
      id: same
      title: B
      query: q
      unit: count");
    assert!(codes(&yaml).contains(&"E-011"));

    let vars = "version: 1\nid: d\ntitle: T\nvariables:\n  - name: ns\n    source: { metric: m, label: l }\n  - name: ns\n    source: { metric: m, label: l }\nelements:\n  - type: stat\n    title: T\n    query: count($ns)\n    unit: count\n";
    assert!(codes(vars).contains(&"E-011"));
}

#[test]
fn e012_both_or_neither_sections_and_elements() {
    let both = "version: 1\nid: d\ntitle: T\nsections: []\nelements: []\n";
    assert!(codes(both).contains(&"E-012"));
    assert!(codes("version: 1\nid: d\ntitle: T\n").contains(&"E-012"));
}

#[test]
fn e013_malformed_label_template() {
    for bad in ["{{pod", "{{}}", "{{a b}}", "}}pod"] {
        let yaml = doc(&format!(
            "
    - type: list
      title: Pods
      query: q
      unit: bytes
      label: \"{bad}\""
        ));
        assert!(
            codes(&yaml).contains(&"E-013"),
            "{bad:?} should be rejected"
        );
    }
}

#[test]
fn e014_thresholds_not_monotonic() {
    let yaml = doc("
    - type: stat
      title: CPU
      query: q
      unit: percent
      thresholds:
        direction: above
        steps:
          - { at: 90, level: critical }
          - { at: 70, level: warning }");
    assert!(codes(&yaml).contains(&"E-014"));
}

#[test]
fn e015_values_out_of_range() {
    let long = "x".repeat(61);
    assert!(codes(&doc(STAT).replace("title: T", &format!("title: {long}"))).contains(&"E-015"));

    let top = doc("
    - type: line
      title: CPU
      query: q
      unit: cores
      series:
        top: 21");
    assert!(codes(&top).contains(&"E-015"));

    let limit = doc("
    - type: list
      title: Pods
      query: q
      unit: bytes
      label: \"{{pod}}\"
      limit: 101");
    assert!(codes(&limit).contains(&"E-015"));

    let empty = "version: 1\nid: d\ntitle: T\nsections:\n  - title: S\n    elements: []\n";
    assert!(codes(empty).contains(&"E-015"));
}

/// The render tree carries no floating-point, so a fractional bound cannot exist.
#[test]
fn a_fractional_bound_is_rejected() {
    let yaml = doc("
    - type: line
      title: Ratio
      query: q
      unit: ratio
      min: 0.5");
    assert!(codes(&yaml).contains(&"E-015"));
}

#[test]
fn w001_unreadable_list() {
    let yaml = doc("
    - type: list
      title: Pods
      query: q
      unit: bytes
      label: \"{{pod}}\"
      limit: 100");
    assert!(codes(&yaml).contains(&"W-001"));
}

#[test]
fn w002_unreferenced_variable() {
    let yaml = "version: 1\nid: d\ntitle: T\nvariables:\n  - name: ns\n    source: { metric: m, label: l }\nelements:\n  - type: stat\n    title: T\n    query: count(kube_pod_info)\n    unit: count\n";
    let (tree, d) = compile_with_diagnostics(yaml);
    assert!(tree.is_some(), "a warning does not fail compilation");
    assert!(d.iter().any(|x| x.code == "W-002"));
}

#[test]
fn w003_long_scroll() {
    let mut body = String::new();
    for i in 0..13 {
        body.push_str(&format!(
            "\n    - type: stat\n      title: S{i}\n      query: q\n      unit: count"
        ));
    }
    assert!(codes(&doc(&body)).contains(&"W-003"));
}

#[test]
fn every_diagnostic_is_reported_not_just_the_first() {
    let yaml = doc("
    - type: stat
      title: A
      query: q
      unit: furlongs
      colour: red");
    let d = diags(&yaml);
    assert!(d.len() >= 2, "expected several diagnostics, got {d:?}");
    assert!(d.iter().any(|x| x.code == "E-007"));
    assert!(d.iter().any(|x| x.code == "E-002"));
}

#[test]
fn a_variable_label_defaults_to_the_titlecased_name() {
    let yaml = "version: 1\nid: d\ntitle: T\nvariables:\n  - name: node_pool\n    source: { metric: m, label: l }\nelements:\n  - type: stat\n    title: T\n    query: count($node_pool)\n    unit: count\n";
    let tree = compile_with_diagnostics(yaml).0.expect("compiles");
    assert_eq!(tree.variables[0].label, "Node pool");
}

/// A known false positive, accepted so the compiler needs no PromQL parser.
#[test]
fn a_variable_in_a_string_literal_is_still_extracted() {
    let yaml = "version: 1\nid: d\ntitle: T\nvariables:\n  - name: ns\n    source: { metric: m, label: l }\nelements:\n  - type: stat\n    title: T\n    query: count(up{note=\"$ns\"})\n    unit: count\n";
    let tree = compile_with_diagnostics(yaml).0.expect("compiles");
    assert_eq!(tree.sections[0].elements[0].query.uses_variables, ["ns"]);
}
