use std::path::PathBuf;

use wiki_core::{DomainSchema, Scope};
use wiki_kernel::{LlmWikiEngine, NoopWikiHook};
use wiki_storage::{SqliteRepository, SqliteWriterLease};

use crate::cli::Cli;
use crate::cli_utils::parse_scope;
use crate::{acquire_cli_writer_lease, cmd_needs_writer_lease, cmd_writer_lease_label};

pub(crate) struct CliRuntime {
    pub(crate) viewer: Scope,
    pub(crate) wiki_root: Option<PathBuf>,
    pub(crate) sync_wiki: bool,
    pub(crate) _writer_lease: Option<SqliteWriterLease>,
    pub(crate) repo: SqliteRepository,
    pub(crate) schema: DomainSchema,
    pub(crate) eng: LlmWikiEngine<NoopWikiHook>,
}

pub(crate) fn open(cli: &Cli) -> Result<CliRuntime, Box<dyn std::error::Error>> {
    let viewer = parse_scope(&cli.viewer_scope);
    let wiki_root = cli.wiki_dir.clone();
    let sync_wiki = cli.sync_wiki;
    let writer_lease = if cmd_needs_writer_lease(&cli.cmd) {
        Some(acquire_cli_writer_lease(
            &cli.db,
            cmd_writer_lease_label(&cli.cmd),
        )?)
    } else {
        None
    };
    let repo = SqliteRepository::open(&cli.db)?;
    let schema = if let Some(path) = &cli.schema {
        DomainSchema::from_json_path(path)?
    } else {
        DomainSchema::permissive_default()
    };
    let eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook)?;

    Ok(CliRuntime {
        viewer,
        wiki_root,
        sync_wiki,
        _writer_lease: writer_lease,
        repo,
        schema,
        eng,
    })
}
