use std::path::PathBuf;
use wiki_core::DomainSchema;

pub(crate) fn run(path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.unwrap_or_else(|| PathBuf::from("DomainSchema.json"));
    match DomainSchema::from_json_path(&path) {
        Ok(schema) => {
            println!(
                "schema ok: title={} lifecycle_rules={}",
                schema.title,
                schema.lifecycle_rules.len()
            );
            Ok(())
        }
        Err(err) => {
            eprintln!("schema invalid: {err}");
            std::process::exit(1);
        }
    }
}
