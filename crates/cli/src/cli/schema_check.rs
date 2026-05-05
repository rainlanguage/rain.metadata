use clap::Parser;
use graphql_parser::schema::{
    parse_schema, Definition, Document, Field, ObjectType, Type, TypeDefinition,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

const INTROSPECTION_QUERY: &str = r#"
{ __schema { types {
  kind name
  fields(includeDeprecated: true) {
    name type { kind name ofType { kind name ofType { kind name ofType { kind name } } } }
  }
} } }
"#;

/// Compare entity types in a subgraph schema against a consumer's snapshot
/// of the deployed introspection schema. Used in deploy CI to fail early
/// when the consumer's snapshot has drifted from what is about to be
/// deployed.
#[derive(Parser)]
pub struct SchemaCheck {
    /// Path to the source subgraph schema (with `@entity` directives).
    /// Mutually exclusive with --live-url.
    #[arg(short, long, conflicts_with = "live_url")]
    pub source: Option<PathBuf>,
    /// URL of a live deployed subgraph endpoint. Introspection is fetched
    /// and used as the source of truth in place of --source. The full
    /// introspection-derived SDL of the entity types is printed on
    /// failure so the consumer snapshot can be regenerated.
    #[arg(short, long, conflicts_with = "source")]
    pub live_url: Option<String>,
    /// Path to the consumer's snapshot of the deployed introspection schema.
    #[arg(short, long)]
    pub consumer: PathBuf,
}

pub async fn schema_check(cmd: SchemaCheck) -> anyhow::Result<()> {
    let consumer_sdl = std::fs::read_to_string(&cmd.consumer)?;

    let (source_sdl, source_label) = match (&cmd.source, &cmd.live_url) {
        (Some(path), None) => (std::fs::read_to_string(path)?, "source".to_string()),
        (None, Some(url)) => {
            let sdl = fetch_live_entities_as_sdl(url).await?;
            (sdl, format!("live ({url})"))
        }
        _ => {
            return Err(anyhow::anyhow!(
                "exactly one of --source or --live-url must be provided"
            ));
        }
    };

    match check(&source_sdl, &consumer_sdl) {
        Ok(count) => {
            println!("schema check ok: {count} entities verified against {source_label}");
            Ok(())
        }
        Err(errors) => {
            let mut msg = format!("schema check failed with {} mismatches:", errors.len());
            for e in &errors {
                msg.push_str("\n  - ");
                msg.push_str(e);
            }
            if cmd.live_url.is_some() {
                msg.push_str(
                    "\n\nLive introspection-derived entity SDL (copy into consumer file):\n",
                );
                msg.push_str(&source_sdl);
            }
            Err(anyhow::anyhow!(msg))
        }
    }
}

/// POST a GraphQL introspection query to `url` and reduce the response to
/// a synthetic SDL document containing only entity Object types and their
/// fields. The synthetic SDL re-tags each type with `@entity` so the
/// existing `entities` filter picks them up.
async fn fetch_live_entities_as_sdl(url: &str) -> anyhow::Result<String> {
    // Bound the request so a slow or hung Goldsky endpoint can't wedge
    // the deploy job indefinitely. reqwest's wasm impl uses the browser
    // fetch API and doesn't expose ClientBuilder timing methods, so the
    // bound is native-only. The CLI binary never runs under wasm.
    #[cfg(not(target_family = "wasm"))]
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    #[cfg(target_family = "wasm")]
    let client = reqwest::Client::new();

    let body = serde_json::json!({ "query": INTROSPECTION_QUERY });
    let resp: serde_json::Value = client
        .post(url)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if let Some(errors) = resp.get("errors") {
        return Err(anyhow::anyhow!("introspection errors: {errors}"));
    }
    let types = resp
        .pointer("/data/__schema/types")
        .and_then(|t| t.as_array())
        .ok_or_else(|| anyhow::anyhow!("introspection response missing /data/__schema/types"))?;

    let mut sdl = String::new();
    for t in types {
        let kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if kind != "OBJECT" || !is_entity_object(name) {
            continue;
        }
        sdl.push_str(&format!("type {name} @entity {{\n"));
        if let Some(fields) = t.get("fields").and_then(|f| f.as_array()) {
            for f in fields {
                let fname = f.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let ftype = render_type(f.get("type").unwrap_or(&serde_json::Value::Null));
                sdl.push_str(&format!("  {fname}: {ftype}\n"));
            }
        }
        sdl.push_str("}\n\n");
    }
    Ok(sdl)
}

/// Filter for "entity-shaped" Object types in a Graph Protocol introspection
/// response: skip auto-generated derivative types (filter, orderBy, Query,
/// Subscription, _Meta_, _Block_, etc.) and any name with a leading underscore.
fn is_entity_object(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('_')
        && name != "Query"
        && name != "Subscription"
        && !name.ends_with("_filter")
        && !name.ends_with("_orderBy")
}

/// Render an introspection type-ref into SDL syntax (`Bytes!`, `[Foo!]!`).
fn render_type(t: &serde_json::Value) -> String {
    let kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let name = t.get("name").and_then(|v| v.as_str());
    let of_type = t.get("ofType");
    match kind {
        "NON_NULL" => format!(
            "{}!",
            render_type(of_type.unwrap_or(&serde_json::Value::Null))
        ),
        "LIST" => format!(
            "[{}]",
            render_type(of_type.unwrap_or(&serde_json::Value::Null))
        ),
        _ => name.unwrap_or("Unknown").to_string(),
    }
}

fn check(source_sdl: &str, consumer_sdl: &str) -> Result<usize, Vec<String>> {
    let source_doc: Document<String> =
        parse_schema(source_sdl).map_err(|e| vec![format!("parse source: {e}")])?;
    let consumer_doc: Document<String> =
        parse_schema(consumer_sdl).map_err(|e| vec![format!("parse consumer: {e}")])?;

    let source_entities = entities(&source_doc);
    let consumer_objects = all_objects(&consumer_doc);

    if source_entities.is_empty() {
        return Err(vec![
            "source schema has no `@entity` types; check that --source/--live-url \
             points at a subgraph SDL or live introspection endpoint"
                .to_string(),
        ]);
    }

    let mut errors = Vec::new();

    for entity in &source_entities {
        match consumer_objects.get(entity.name.as_str()) {
            None => errors.push(format!(
                "entity `{}` is missing from consumer schema",
                entity.name
            )),
            Some(consumer_entity) => {
                let consumer_fields: BTreeMap<&str, &Field<'_, String>> = consumer_entity
                    .fields
                    .iter()
                    .map(|f| (f.name.as_str(), f))
                    .collect();
                for field in &entity.fields {
                    match consumer_fields.get(field.name.as_str()) {
                        None => errors.push(format!(
                            "field `{}.{}` is missing from consumer schema",
                            entity.name, field.name
                        )),
                        Some(consumer_field) => {
                            if !type_equal(&field.field_type, &consumer_field.field_type) {
                                errors.push(format!(
                                    "field `{}.{}` type mismatch: source `{}` vs consumer `{}`",
                                    entity.name,
                                    field.name,
                                    type_to_string(&field.field_type),
                                    type_to_string(&consumer_field.field_type),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(source_entities.len())
    } else {
        Err(errors)
    }
}

fn entities<'a>(doc: &'a Document<'a, String>) -> Vec<&'a ObjectType<'a, String>> {
    doc.definitions
        .iter()
        .filter_map(|def| {
            if let Definition::TypeDefinition(TypeDefinition::Object(obj)) = def {
                if obj.directives.iter().any(|d| d.name == "entity") {
                    return Some(obj);
                }
            }
            None
        })
        .collect()
}

fn all_objects<'a>(doc: &'a Document<'a, String>) -> BTreeMap<&'a str, &'a ObjectType<'a, String>> {
    doc.definitions
        .iter()
        .filter_map(|def| {
            if let Definition::TypeDefinition(TypeDefinition::Object(obj)) = def {
                Some((obj.name.as_str(), obj))
            } else {
                None
            }
        })
        .collect()
}

fn type_equal(a: &Type<'_, String>, b: &Type<'_, String>) -> bool {
    match (a, b) {
        (Type::NamedType(an), Type::NamedType(bn)) => an == bn,
        (Type::ListType(ai), Type::ListType(bi)) => type_equal(ai, bi),
        (Type::NonNullType(ai), Type::NonNullType(bi)) => type_equal(ai, bi),
        _ => false,
    }
}

fn type_to_string(t: &Type<'_, String>) -> String {
    match t {
        Type::NamedType(n) => n.clone(),
        Type::ListType(inner) => format!("[{}]", type_to_string(inner)),
        Type::NonNullType(inner) => format!("{}!", type_to_string(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE_OK: &str = r#"
        type MetaBoard @entity {
            id: Bytes!
            address: Bytes!
            nextMetaId: BigInt!
        }
        type MetaV1 @entity {
            id: ID!
            sender: Bytes!
            subject: Bytes!
        }
    "#;

    const CONSUMER_OK: &str = r#"
        type MetaBoard {
          id: Bytes!
          address: Bytes!
          nextMetaId: BigInt!
        }
        type MetaV1 {
          id: ID!
          sender: Bytes!
          subject: Bytes!
        }
    "#;

    #[test]
    fn matching_schemas_pass() {
        let n = check(SOURCE_OK, CONSUMER_OK).unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn missing_entity_is_reported() {
        let consumer = r#"
            type MetaBoard {
              id: Bytes!
              address: Bytes!
              nextMetaId: BigInt!
            }
        "#;
        let errs = check(SOURCE_OK, consumer).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("entity `MetaV1` is missing"));
    }

    #[test]
    fn missing_field_is_reported() {
        let consumer = r#"
            type MetaBoard {
              id: Bytes!
              address: Bytes!
              nextMetaId: BigInt!
            }
            type MetaV1 {
              id: ID!
              sender: Bytes!
            }
        "#;
        let errs = check(SOURCE_OK, consumer).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("field `MetaV1.subject` is missing"));
    }

    #[test]
    fn type_mismatch_is_reported() {
        let consumer = r#"
            type MetaBoard {
              id: Bytes!
              address: Bytes!
              nextMetaId: BigInt!
            }
            type MetaV1 {
              id: ID!
              sender: Bytes!
              subject: BigInt!
            }
        "#;
        let errs = check(SOURCE_OK, consumer).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("`MetaV1.subject` type mismatch"));
        assert!(errs[0].contains("source `Bytes!`"));
        assert!(errs[0].contains("consumer `BigInt!`"));
    }

    #[test]
    fn deployed_subgraph_drift_is_caught() {
        // Mirrors the actual divergence between subgraph/schema.graphql
        // (source) and crates/metaboard/src/schema/metaboard.graphql
        // (consumer) at the time this subcommand was added: missing
        // `Transaction` entity and `MetaV1.transaction`, plus `subject`
        // type mismatch (Bytes! vs BigInt!).
        let source = r#"
            type MetaBoard @entity {
                id: Bytes!
                address: Bytes!
                nextMetaId: BigInt!
            }
            type MetaV1 @entity {
                id: ID!
                transaction: Transaction!
                metaBoard: MetaBoard!
                sender: Bytes!
                subject: Bytes!
                metaHash: Bytes!
                meta: Bytes!
            }
            type Transaction @entity(immutable: true) {
                id: Bytes!
                timestamp: BigInt!
                blockNumber: BigInt!
                from: Bytes!
            }
        "#;
        let consumer = r#"
            type MetaBoard {
              id: Bytes!
              address: Bytes!
              nextMetaId: BigInt!
            }
            type MetaV1 {
              id: ID!
              metaBoard: MetaBoard!
              sender: Bytes!
              subject: BigInt!
              metaHash: Bytes!
              meta: Bytes!
            }
        "#;
        let errs = check(source, consumer).unwrap_err();
        assert!(errs
            .iter()
            .any(|e| e.contains("entity `Transaction` is missing")));
        assert!(errs
            .iter()
            .any(|e| e.contains("field `MetaV1.transaction` is missing")));
        assert!(errs
            .iter()
            .any(|e| e.contains("`MetaV1.subject` type mismatch")));
    }

    #[test]
    fn source_with_no_entities_is_an_error() {
        // Silently passing with 0 entities verified would mask a
        // misconfigured --source path or a non-subgraph SDL in CI.
        let source = "scalar Bytes";
        let consumer = "type Whatever { x: Int }";
        let errs = check(source, consumer).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("no `@entity` types"));
    }

    #[test]
    fn non_object_definitions_in_source_are_ignored() {
        // Enums, scalars, and Object types without `@entity` must not be
        // treated as entities to check.
        let source = r#"
            scalar Bytes
            enum Direction { ASC DESC }
            type NotAnEntity {
                noisefield: Int
            }
            type MetaBoard @entity {
                id: Bytes!
            }
        "#;
        let consumer = r#"
            type MetaBoard {
              id: Bytes!
            }
        "#;
        let n = check(source, consumer).unwrap();
        assert_eq!(n, 1, "only the @entity-tagged type should be verified");
    }

    #[test]
    fn entity_directive_with_arguments_is_detected() {
        // `@entity(immutable: true)` must still be picked up.
        let source = r#"
            type Transaction @entity(immutable: true) {
                id: Bytes!
            }
        "#;
        let consumer = r#"
            type Transaction {
              id: Bytes!
            }
        "#;
        let n = check(source, consumer).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn consumer_extras_are_ignored() {
        // The consumer schema is the introspected GraphQL service surface
        // and contains derivative types (filters, orderBy) plus extra
        // fields with arguments. Those must not cause errors.
        let consumer = r#"
            type MetaBoard {
              id: Bytes!
              address: Bytes!
              nextMetaId: BigInt!
              extraField: String
            }
            type MetaV1 {
              id: ID!
              sender: Bytes!
              subject: Bytes!
            }
            input MetaBoard_filter {
              id: Bytes
            }
            enum MetaBoard_orderBy { id address }
        "#;
        let n = check(SOURCE_OK, consumer).unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn consumer_field_with_arguments_matches_when_return_type_matches() {
        // Introspected derived fields look like `metas(skip: Int = 0, ...): [MetaV1!]`.
        // We compare only the return type, not the args, so this must pass.
        let source = r#"
            type MetaBoard @entity {
                id: Bytes!
                metas: [MetaV1!]
            }
            type MetaV1 @entity {
                id: ID!
            }
        "#;
        let consumer = r#"
            type MetaBoard {
              id: Bytes!
              metas(skip: Int = 0, first: Int = 100): [MetaV1!]
            }
            type MetaV1 {
              id: ID!
            }
        "#;
        let n = check(source, consumer).unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn nullability_mismatch_is_reported() {
        // `Bytes` (nullable) vs `Bytes!` (non-null) are distinct types
        // and must be flagged.
        let source = r#"
            type MetaBoard @entity {
                id: Bytes!
            }
        "#;
        let consumer = r#"
            type MetaBoard {
              id: Bytes
            }
        "#;
        let errs = check(source, consumer).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("source `Bytes!`"));
        assert!(errs[0].contains("consumer `Bytes`"));
    }

    #[test]
    fn list_vs_scalar_mismatch_is_reported() {
        let source = r#"
            type MetaBoard @entity {
                metas: [MetaV1!]
            }
            type MetaV1 @entity {
                id: ID!
            }
        "#;
        let consumer = r#"
            type MetaBoard {
              metas: MetaV1
            }
            type MetaV1 {
              id: ID!
            }
        "#;
        let errs = check(source, consumer).unwrap_err();
        assert!(errs
            .iter()
            .any(|e| e.contains("`MetaBoard.metas` type mismatch")));
        assert!(errs.iter().any(|e| e.contains("source `[MetaV1!]`")));
    }

    #[test]
    fn nested_wrapper_types_compare_recursively() {
        // `[Bytes!]!` must match `[Bytes!]!` exactly and differ from `[Bytes!]`.
        let source = r#"
            type MetaBoard @entity {
                tags: [Bytes!]!
            }
        "#;
        let ok_consumer = r#"
            type MetaBoard {
              tags: [Bytes!]!
            }
        "#;
        let n = check(source, ok_consumer).unwrap();
        assert_eq!(n, 1);

        let bad_consumer = r#"
            type MetaBoard {
              tags: [Bytes!]
            }
        "#;
        let errs = check(source, bad_consumer).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("source `[Bytes!]!`"));
        assert!(errs[0].contains("consumer `[Bytes!]`"));
    }

    #[test]
    fn multiple_errors_are_all_reported() {
        // One run should surface every problem, not stop at the first.
        let source = r#"
            type MetaBoard @entity {
                id: Bytes!
                address: Bytes!
                nextMetaId: BigInt!
            }
            type MetaV1 @entity {
                id: ID!
                sender: Bytes!
                subject: Bytes!
            }
            type Transaction @entity {
                id: Bytes!
            }
        "#;
        let consumer = r#"
            type MetaBoard {
              id: Bytes!
              address: BigInt!
            }
            type MetaV1 {
              id: ID!
              sender: Bytes!
            }
        "#;
        let errs = check(source, consumer).unwrap_err();
        // MetaBoard.address mismatch + MetaBoard.nextMetaId missing
        // + MetaV1.subject missing + Transaction entity missing = 4
        assert_eq!(errs.len(), 4, "errors were: {:?}", errs);
    }

    #[test]
    fn unparseable_source_is_reported() {
        let errs = check("type Broken @entity {", CONSUMER_OK).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].starts_with("parse source:"));
    }

    #[test]
    fn unparseable_consumer_is_reported() {
        let errs = check(SOURCE_OK, "type Broken {").unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].starts_with("parse consumer:"));
    }

    #[test]
    fn consumer_with_no_objects_reports_every_source_entity_missing() {
        // Use a valid-but-Object-free schema (graphql-parser rejects fully
        // empty input as a parse error, which is its own test case).
        let errs = check(SOURCE_OK, "scalar Whatever").unwrap_err();
        // SOURCE_OK has 2 entities (MetaBoard, MetaV1).
        assert_eq!(errs.len(), 2);
        assert!(errs
            .iter()
            .any(|e| e.contains("entity `MetaBoard` is missing")));
        assert!(errs
            .iter()
            .any(|e| e.contains("entity `MetaV1` is missing")));
    }

    // ---------- helper-function unit tests ----------

    fn parse(sdl: &str) -> Document<'_, String> {
        parse_schema(sdl).unwrap()
    }

    #[test]
    fn entities_returns_only_entity_directive_objects() {
        let doc = parse(
            r#"
            type WithEntity @entity { id: ID! }
            type WithEntityArgs @entity(immutable: true) { id: ID! }
            type Plain { id: ID! }
            type WithOtherDirective @other { id: ID! }
            scalar S
            enum E { A B }
            "#,
        );
        let names: Vec<&str> = entities(&doc).iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, vec!["WithEntity", "WithEntityArgs"]);
    }

    #[test]
    fn all_objects_returns_every_object_type_keyed_by_name() {
        let doc = parse(
            r#"
            type A { x: Int }
            type B @entity { y: Int }
            scalar S
            enum E { X }
            input I { z: Int }
            "#,
        );
        let m = all_objects(&doc);
        let mut names: Vec<&str> = m.keys().copied().collect();
        names.sort();
        assert_eq!(names, vec!["A", "B"]);
    }

    fn named(s: &str) -> Type<'static, String> {
        Type::NamedType(s.to_string())
    }
    fn nn(t: Type<'static, String>) -> Type<'static, String> {
        Type::NonNullType(Box::new(t))
    }
    fn list(t: Type<'static, String>) -> Type<'static, String> {
        Type::ListType(Box::new(t))
    }

    #[test]
    fn type_equal_named_named() {
        assert!(type_equal(&named("Bytes"), &named("Bytes")));
        assert!(!type_equal(&named("Bytes"), &named("BigInt")));
    }

    #[test]
    fn type_equal_distinguishes_wrappers() {
        assert!(!type_equal(&named("Bytes"), &nn(named("Bytes"))));
        assert!(!type_equal(&named("Bytes"), &list(named("Bytes"))));
        assert!(!type_equal(&nn(named("Bytes")), &list(named("Bytes"))));
    }

    #[test]
    fn type_equal_recurses_through_nested_wrappers() {
        let a = nn(list(nn(named("Bytes"))));
        let b = nn(list(nn(named("Bytes"))));
        assert!(type_equal(&a, &b));
        let c = nn(list(named("Bytes")));
        assert!(!type_equal(&a, &c));
    }

    #[test]
    fn type_to_string_renders_sdl_syntax() {
        assert_eq!(type_to_string(&named("Bytes")), "Bytes");
        assert_eq!(type_to_string(&nn(named("Bytes"))), "Bytes!");
        assert_eq!(type_to_string(&list(named("X"))), "[X]");
        assert_eq!(type_to_string(&nn(list(nn(named("X"))))), "[X!]!");
    }

    #[test]
    fn is_entity_object_skips_derivative_and_internal_types() {
        assert!(is_entity_object("MetaBoard"));
        assert!(is_entity_object("Transaction"));
        assert!(!is_entity_object(""));
        assert!(!is_entity_object("_Meta_"));
        assert!(!is_entity_object("Query"));
        assert!(!is_entity_object("Subscription"));
        assert!(!is_entity_object("MetaV1_filter"));
        assert!(!is_entity_object("MetaV1_orderBy"));
    }

    #[test]
    fn render_type_unwraps_introspection_typeref_recursively() {
        // NON_NULL[LIST[NON_NULL[Bytes]]] → "[Bytes!]!"
        let nested = serde_json::json!({
            "kind": "NON_NULL",
            "name": null,
            "ofType": {
                "kind": "LIST",
                "name": null,
                "ofType": {
                    "kind": "NON_NULL",
                    "name": null,
                    "ofType": { "kind": "SCALAR", "name": "Bytes", "ofType": null }
                }
            }
        });
        assert_eq!(render_type(&nested), "[Bytes!]!");
    }

    #[test]
    fn render_type_handles_plain_named_type() {
        let scalar = serde_json::json!({ "kind": "SCALAR", "name": "BigInt", "ofType": null });
        assert_eq!(render_type(&scalar), "BigInt");
    }

    #[test]
    fn render_type_falls_back_to_unknown_for_missing_name() {
        let bad = serde_json::json!({ "kind": "SCALAR", "name": null, "ofType": null });
        assert_eq!(render_type(&bad), "Unknown");
    }

    // ---------- live-URL HTTP path ----------

    #[tokio::test]
    async fn fetch_live_entities_filters_to_entity_object_types() {
        use httpmock::Method::POST;
        use httpmock::MockServer;

        let server = MockServer::start_async().await;
        let _mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/");
                then.status(200).json_body(serde_json::json!({
                "data": { "__schema": { "types": [
                    { "kind": "OBJECT", "name": "MetaBoard", "fields": [
                        { "name": "id", "type": { "kind": "NON_NULL", "name": null,
                            "ofType": { "kind": "SCALAR", "name": "Bytes", "ofType": null } } }
                    ] },
                    { "kind": "OBJECT", "name": "MetaV1_filter", "fields": [
                        { "name": "id", "type": { "kind": "SCALAR", "name": "ID", "ofType": null } }
                    ] },
                    { "kind": "OBJECT", "name": "Query", "fields": [] },
                    { "kind": "OBJECT", "name": "_Meta_", "fields": [] },
                    { "kind": "SCALAR", "name": "Bytes", "fields": null }
                ] } }
            }));
            })
            .await;

        let sdl = fetch_live_entities_as_sdl(&server.url("/")).await.unwrap();
        assert!(sdl.contains("type MetaBoard @entity"));
        assert!(sdl.contains("id: Bytes!"));
        assert!(!sdl.contains("MetaV1_filter"));
        assert!(!sdl.contains("Query"));
        assert!(!sdl.contains("_Meta_"));
    }

    #[tokio::test]
    async fn fetch_live_entities_propagates_graphql_errors() {
        use httpmock::Method::POST;
        use httpmock::MockServer;

        let server = MockServer::start_async().await;
        let _mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/");
                then.status(200).json_body(serde_json::json!({
                    "errors": [{ "message": "introspection disabled" }]
                }));
            })
            .await;

        let err = fetch_live_entities_as_sdl(&server.url("/"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("introspection errors"));
        assert!(err.to_string().contains("introspection disabled"));
    }

    #[tokio::test]
    async fn fetch_live_entities_errors_on_malformed_response() {
        use httpmock::Method::POST;
        use httpmock::MockServer;

        let server = MockServer::start_async().await;
        let _mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/");
                then.status(200)
                    .json_body(serde_json::json!({ "data": {} }));
            })
            .await;

        let err = fetch_live_entities_as_sdl(&server.url("/"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("missing /data/__schema/types"));
    }

    #[tokio::test]
    async fn fetch_live_entities_errors_on_http_failure() {
        use httpmock::Method::POST;
        use httpmock::MockServer;

        let server = MockServer::start_async().await;
        let _mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/");
                then.status(500);
            })
            .await;

        let err = fetch_live_entities_as_sdl(&server.url("/"))
            .await
            .unwrap_err();
        // reqwest's error_for_status produces "500 Internal Server Error" text.
        assert!(err.to_string().contains("500"));
    }

    // ---------- end-to-end CLI handler ----------

    #[tokio::test]
    async fn schema_check_reads_files_and_succeeds_on_match() {
        use std::io::Write;
        let mut src = tempfile::NamedTempFile::new().unwrap();
        src.write_all(SOURCE_OK.as_bytes()).unwrap();
        let mut con = tempfile::NamedTempFile::new().unwrap();
        con.write_all(CONSUMER_OK.as_bytes()).unwrap();

        schema_check(SchemaCheck {
            source: Some(src.path().into()),
            live_url: None,
            consumer: con.path().into(),
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn schema_check_rejects_neither_source_nor_live_url() {
        let mut con = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut con, CONSUMER_OK.as_bytes()).unwrap();

        let err = schema_check(SchemaCheck {
            source: None,
            live_url: None,
            consumer: con.path().into(),
        })
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("exactly one of --source or --live-url"));
    }

    #[tokio::test]
    async fn schema_check_failure_includes_live_sdl_in_error() {
        use httpmock::Method::POST;
        use httpmock::MockServer;
        use std::io::Write;

        // Live introspection returns a single MetaBoard entity; consumer
        // file omits it, so the error should include the live-derived SDL.
        let server = MockServer::start_async().await;
        let _mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/");
                then.status(200).json_body(serde_json::json!({
                    "data": { "__schema": { "types": [
                        { "kind": "OBJECT", "name": "MetaBoard", "fields": [
                            { "name": "id", "type": { "kind": "NON_NULL", "name": null,
                                "ofType": { "kind": "SCALAR", "name": "Bytes", "ofType": null } } }
                        ] }
                    ] } }
                }));
            })
            .await;

        let mut con = tempfile::NamedTempFile::new().unwrap();
        con.write_all(b"scalar X").unwrap();

        let err = schema_check(SchemaCheck {
            source: None,
            live_url: Some(server.url("/")),
            consumer: con.path().into(),
        })
        .await
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("entity `MetaBoard` is missing"));
        assert!(msg.contains("Live introspection-derived entity SDL"));
        assert!(msg.contains("type MetaBoard @entity"));
    }
}
