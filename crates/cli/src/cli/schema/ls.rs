use crate::meta::KnownMeta;
use strum::IntoEnumIterator;

pub fn ls() -> anyhow::Result<()> {
    for schema in KnownMeta::iter().filter(|meta| super::json_schema(*meta).is_some()) {
        println!("{}", schema);
    }
    Ok(())
}
