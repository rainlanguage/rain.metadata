use clap::Parser;
use graphql_parser::schema::{
    parse_schema, Definition, Document, Field, ObjectType, Type, TypeDefinition,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Compare entity types in a subgraph schema against a consumer's snapshot
/// of the deployed introspection schema. Used in deploy CI to fail early
/// when the consumer's snapshot has drifted from what is about to be
/// deployed.
#[derive(Parser)]
pub struct SchemaCheck {
    /// Path to the source subgraph schema (with `@entity` directives).
    #[arg(short, long)]
    pub source: PathBuf,
    /// Path to the consumer's snapshot of the deployed introspection schema.
    #[arg(short, long)]
    pub consumer: PathBuf,
}

pub fn schema_check(cmd: SchemaCheck) -> anyhow::Result<()> {
    let source_sdl = std::fs::read_to_string(&cmd.source)?;
    let consumer_sdl = std::fs::read_to_string(&cmd.consumer)?;

    match check(&source_sdl, &consumer_sdl) {
        Ok(count) => {
            println!("schema check ok: {count} entities verified");
            Ok(())
        }
        Err(errors) => {
            let mut msg = format!("schema check failed with {} mismatches:", errors.len());
            for e in &errors {
                msg.push_str("\n  - ");
                msg.push_str(e);
            }
            Err(anyhow::anyhow!(msg))
        }
    }
}

fn check(source_sdl: &str, consumer_sdl: &str) -> Result<usize, Vec<String>> {
    let source_doc: Document<String> =
        parse_schema(source_sdl).map_err(|e| vec![format!("parse source: {e}")])?;
    let consumer_doc: Document<String> =
        parse_schema(consumer_sdl).map_err(|e| vec![format!("parse consumer: {e}")])?;

    let source_entities = entities(&source_doc);
    let consumer_objects = all_objects(&consumer_doc);

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
}
