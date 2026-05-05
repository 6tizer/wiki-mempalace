pub fn run_mcp(
    db_path: &std::path::Path,
    schema: wiki_core::DomainSchema,
    viewer_scope: &str,
    once: bool,
    llm_config_path: &std::path::Path,
    vectors: bool,
    wiki_dir: Option<&std::path::Path>,
    palace_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    wiki_tools::run_mcp(
        db_path,
        schema,
        viewer_scope,
        once,
        llm_config_path,
        vectors,
        wiki_dir,
        palace_path,
    )
}
