# dashboard-dsl

A dashboard is a YAML document. This compiles it into a render tree — a fully
resolved JSON artifact with no defaults left to work out and no geometry to
interpret.

```sh
vedavid-dash compile dashboards/namespace.yaml -o namespace.json
vedavid-dash lint dashboards/ --deny-warnings
vedavid-dash schema render-tree
```

The compiler is a pure function of its input:

```rust
vedavid_dashboard_dsl::compile(yaml) -> Result<RenderTree, Vec<Diagnostic>>
```

No filesystem, no network, no clock. Identical input gives byte-identical
output, which is what lets it run in a customer's CI with no credentials,
compile to `wasm32-unknown-unknown` for browser preview, and be tested by
golden file. All filesystem access lives in the `vedavid-dash` binary.

## Writing a dashboard

```yaml
version: 1
id: k8s-namespace          # identity comes from this field, never the filename
title: Namespace
variables:
  - name: namespace
    source:
      metric: kube_pod_info
      label: namespace
sections:
  - title: Compute
    elements:
      - type: line
        title: CPU by pod
        query: sum by (pod) (rate(container_cpu_usage_seconds_total{namespace="$namespace"}[5m]))
        unit: cores
        series:
          label: "{{pod}}"
          top: 5
```

Use `elements:` at the top level instead of `sections:` for a single-section
dashboard; it compiles to one untitled section.

There is no size, position or column anywhere. Layout is an ordered
full-width vertical scroll, and the order in the document is the order on
screen.

### Element types

| type | reads | shows |
| --- | --- | --- |
| `line` | a range query | one line per series over time |
| `stat` | an instant query returning one series | a single number |
| `list` | an instant query | a labelled row per series |

`area` is `line` with `stack: true`, because stacking says something about the
data — part-to-whole rather than independent series — rather than about the
chart.

Every element needs a `unit`, because PromQL returns bare floats and nothing
downstream can guess whether `1.4e9` is bytes or requests per second.

### Thresholds

```yaml
thresholds:
  direction: above       # above = larger is worse
  steps:
    - { at: 70, level: warning }
    - { at: 90, level: critical }
```

`level` is `ok`, `warning` or `critical` — the semantic name, not a colour.
Steps must already be ordered in the declared direction; a misordered list is
rejected rather than sorted, because it usually means something else was
intended.

## Diagnostics

Every diagnostic for a document is reported in one run, not just the first.
Each carries a stable code, a message and a `path` locating it in the source:

```
error E-010: query references undeclared variable $cluster [elements[0].query]
```

`E-` codes fail compilation, `W-` codes do not. `lint` exits 0 with warnings
unless given `--deny-warnings`, which is what CI should use.

An unknown field is an error rather than something ignored. That is what makes
adding a field to the language safe: no existing document can contain a field a
newer compiler does not know, and no newer document loses meaning silently on
an older one — it is rejected instead.

## Versions

Three numbers move independently:

| Version | Where | Meaning |
| --- | --- | --- |
| `version` | in the YAML | which input grammar the author wrote |
| `schema` | in the output JSON | which output contract consumers must handle |
| crate version | `Cargo.toml` | semver of the library API |

The `compiler` field in the output records which build produced it. It is
excluded from `hash`, so a patch release that changes no output does not
invalidate a cache.

## Conformance

`conformance/` holds input documents beside their expected output. It is
published with each release so that a reimplementation in another language can
be checked against the same fixtures, and so that changing compiler output
cannot happen by accident.

```sh
cargo test                          # run the corpus
UPDATE_EXPECT=1 cargo test          # rewrite expectations, then read the diff
```

Every bug fix adds a case. Every new field adds a case.

## Not generated yet

`vedavid-dash schema dsl` is unimplemented. The validator walks a generic YAML
value rather than deserializing into typed structs — that is what allows every
diagnostic to be reported in one run instead of stopping at the first — so
there are no serde types for the input grammar to generate a schema from.
Adding mirror types for schema generation alone would let the schema drift
silently from the validator, which is worse than not shipping one.
`render-tree` is generated from the real types and is checked in CI.

## Licence

Apache-2.0.
