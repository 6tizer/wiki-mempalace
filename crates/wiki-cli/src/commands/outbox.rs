use wiki_storage::{SqliteRepository, WikiRepository};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConsumerOutboxNdjson {
    pub(crate) start_id: i64,
    pub(crate) head_id: i64,
    pub(crate) ndjson: String,
}

pub(crate) fn run_export_all(repo: &SqliteRepository) -> Result<(), Box<dyn std::error::Error>> {
    print!("{}", repo.export_outbox_ndjson()?);
    Ok(())
}

pub(crate) fn run_export_from(
    repo: &SqliteRepository,
    consumer_tag: &str,
    last_id: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let export = export_outbox_ndjson_for_consumer_floor(repo, consumer_tag, last_id)?;
    print!("{}", export.ndjson);
    Ok(())
}

pub(crate) fn run_ack(
    repo: &SqliteRepository,
    up_to_id: i64,
    consumer_tag: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let n = repo.mark_outbox_processed(up_to_id, consumer_tag)?;
    println!("acked={n}");
    Ok(())
}

pub(crate) fn export_outbox_ndjson_for_consumer_floor(
    repo: &SqliteRepository,
    consumer_tag: &str,
    last_id: i64,
) -> Result<ConsumerOutboxNdjson, Box<dyn std::error::Error>> {
    let export = repo.export_outbox_ndjson_for_consumer(consumer_tag)?;
    let start_id = effective_cursor_start_id(export.start_after_id, last_id);
    let ndjson = if start_id == export.start_after_id {
        export.ndjson
    } else {
        repo.export_outbox_ndjson_from_id(start_id)?
    };
    Ok(ConsumerOutboxNdjson {
        start_id,
        head_id: export.head_id,
        ndjson,
    })
}

fn effective_cursor_start_id(cursor_start_after_id: i64, last_id_floor: i64) -> i64 {
    cursor_start_after_id.max(last_id_floor)
}

#[cfg(test)]
mod tests {
    use super::effective_cursor_start_id;

    #[test]
    fn cursor_start_id_respects_last_id_floor() {
        assert_eq!(effective_cursor_start_id(10, 0), 10);
        assert_eq!(effective_cursor_start_id(10, 4), 10);
        assert_eq!(effective_cursor_start_id(10, 20), 20);
    }
}
