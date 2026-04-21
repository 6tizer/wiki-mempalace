use crate::service::{
    drawer_content, extract_to_kg, kg_query, kg_stats, kg_timeline, reflect_answer,
    search_with_options, status, taxonomy, traverse, wake_up, AppConfig, Palace,
};
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

pub fn run_stdio(palace: &Palace, once: bool, quiet: bool, config: &AppConfig) -> Result<()> {
    palace.init(None)?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut handle = stdin.lock();
    let mut line = String::new();

    loop {
        line.clear();
        let n = handle.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        let req: Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(e) => {
                writeln!(
                    stdout,
                    "{}",
                    json!({"jsonrpc":"2.0","id":Value::Null,"error":{"code":-32700,"message":e.to_string()}})
                )?;
                stdout.flush()?;
                if once {
                    break;
                }
                continue;
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

        let resp = handle_request(palace, method, params, id, config, quiet);
        writeln!(stdout, "{resp}")?;
        stdout.flush()?;
        if once {
            break;
        }
    }
    Ok(())
}

fn handle_request(
    palace: &Palace,
    method: &str,
    params: Value,
    id: Value,
    config: &AppConfig,
    _quiet: bool,
) -> Value {
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion":"2024-11-05",
            "serverInfo":{"name":"rust-mempalace","version":"0.1.0"},
            "capabilities":{"tools":{}}
        })),
        "tools/list" => Ok(json!({
            "tools":[
                {"name":"mempalace_status","description":"Palace overview","inputSchema":{"type":"object","properties":{}}},
                {"name":"mempalace_search","description":"Search memories","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"wing":{"type":"string"},"hall":{"type":"string"},"room":{"type":"string"},"bank_id":{"type":"string"},"limit":{"type":"integer"},"explain":{"type":"boolean"}},"required":["query"]}},
                {"name":"mempalace_wake_up","description":"Get wake-up context","inputSchema":{"type":"object","properties":{"wing":{"type":"string"},"bank_id":{"type":"string"}}}},
                {"name":"mempalace_kg_query","description":"Query knowledge graph","inputSchema":{"type":"object","properties":{"subject":{"type":"string"},"as_of":{"type":"string"}},"required":["subject"]}},
                {"name":"mempalace_taxonomy","description":"Get taxonomy (optional bank filter)","inputSchema":{"type":"object","properties":{"bank_id":{"type":"string"}}}},
                {"name":"mempalace_traverse","description":"Traverse room links","inputSchema":{"type":"object","properties":{"wing":{"type":"string"},"room":{"type":"string"},"bank_id":{"type":"string"}},"required":["wing","room"]}},
                {"name":"mempalace_kg_timeline","description":"KG timeline by subject","inputSchema":{"type":"object","properties":{"subject":{"type":"string"}},"required":["subject"]}},
                {"name":"mempalace_kg_stats","description":"KG stats","inputSchema":{"type":"object","properties":{}}},
                {"name":"mempalace_reflect","description":"Reflect on memories via optional LLM","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"search_limit":{"type":"integer"},"bank_id":{"type":"string"}},"required":["query"]}},
                {"name":"mempalace_extract","description":"Extract KG triples from text via optional LLM","inputSchema":{"type":"object","properties":{"text":{"type":"string"},"drawer_id":{"type":"integer"}}}}
            ]
        })),
        "tools/call" => call_tool(palace, params, config),
        _ => Err(anyhow::anyhow!("unknown method: {method}")),
    };

    match result {
        Ok(v) => json!({"jsonrpc":"2.0","id":id,"result":v}),
        Err(e) => json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":e.to_string()}}),
    }
}

fn call_tool(palace: &Palace, params: Value, config: &AppConfig) -> Result<Value> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let conn = palace.open()?;
    match name {
        "mempalace_status" => {
            let s = status(&conn)?;
            Ok(
                json!({"drawers":s.drawers,"wings":s.wings,"tunnels":s.tunnels,"kg_facts":s.kg_facts}),
            )
        }
        "mempalace_search" => {
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("missing query"))?;
            let wing = args.get("wing").and_then(Value::as_str);
            let hall = args.get("hall").and_then(Value::as_str);
            let room = args.get("room").and_then(Value::as_str);
            let bank_id = args.get("bank_id").and_then(Value::as_str);
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(8) as usize;
            let explain = args
                .get("explain")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let rows = search_with_options(
                &conn,
                query,
                wing,
                hall,
                room,
                bank_id,
                limit,
                &config.retrieval,
                explain,
            )?;
            Ok(json!({"results": rows.into_iter().map(|r| json!({
                "id": r.id,
                "wing": r.wing,
                "hall": r.hall,
                "room": r.room,
                "bank_id": r.bank_id,
                "source_path": r.source_path,
                "snippet": r.snippet,
                "score": r.score,
                "explain": r.explain
            })).collect::<Vec<_>>()}))
        }
        "mempalace_wake_up" => {
            let wing = args.get("wing").and_then(Value::as_str);
            let bank_id = args.get("bank_id").and_then(Value::as_str);
            let text = wake_up(&conn, &palace.identity_path, wing, bank_id)?;
            Ok(json!({"text": text}))
        }
        "mempalace_kg_query" => {
            let subject = args
                .get("subject")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("missing subject"))?;
            let as_of = args.get("as_of").and_then(Value::as_str);
            let rows = kg_query(&conn, subject, as_of)?;
            Ok(json!({"facts": rows.into_iter().map(|r| json!({
                "id": r.id,
                "subject": r.subject,
                "predicate": r.predicate,
                "object": r.object,
                "valid_from": r.valid_from,
                "valid_to": r.valid_to,
                "source_drawer_id": r.source_drawer_id
            })).collect::<Vec<_>>()}))
        }
        "mempalace_taxonomy" => {
            let bank_id = args.get("bank_id").and_then(Value::as_str);
            let rows = taxonomy(&conn, bank_id)?;
            Ok(
                json!({"taxonomy": rows.into_iter().map(|r| json!({"wing":r.wing,"hall":r.hall,"room":r.room,"count":r.count})).collect::<Vec<_>>()}),
            )
        }
        "mempalace_traverse" => {
            let wing = args
                .get("wing")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("missing wing"))?;
            let room = args
                .get("room")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("missing room"))?;
            let bank_id = args.get("bank_id").and_then(Value::as_str);
            let rows = traverse(&conn, wing, room, bank_id)?;
            Ok(
                json!({"links": rows.into_iter().map(|r| json!({"kind":r.kind,"from_wing":r.from_wing,"from_room":r.from_room,"to_wing":r.to_wing,"to_room":r.to_room})).collect::<Vec<_>>()}),
            )
        }
        "mempalace_kg_timeline" => {
            let subject = args
                .get("subject")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("missing subject"))?;
            let rows = kg_timeline(&conn, subject)?;
            Ok(json!({"timeline": rows.into_iter().map(|r| json!({
                "id": r.id,
                "subject": r.subject,
                "predicate": r.predicate,
                "object": r.object,
                "valid_from": r.valid_from,
                "valid_to": r.valid_to,
                "source_drawer_id": r.source_drawer_id
            })).collect::<Vec<_>>()}))
        }
        "mempalace_kg_stats" => {
            let s = kg_stats(&conn)?;
            Ok(
                json!({"facts":s.facts,"subjects":s.subjects,"predicates":s.predicates,"active_facts":s.active_facts}),
            )
        }
        "mempalace_reflect" => {
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("missing query"))?;
            let search_limit = args
                .get("search_limit")
                .and_then(Value::as_u64)
                .unwrap_or(8) as usize;
            let bank_id = args.get("bank_id").and_then(Value::as_str);
            let text = reflect_answer(
                &conn,
                &config.llm,
                &config.retrieval,
                query,
                bank_id,
                search_limit,
            )?;
            Ok(json!({"text": text}))
        }
        "mempalace_extract" => {
            let body = match (
                args.get("text").and_then(Value::as_str),
                args.get("drawer_id").and_then(Value::as_i64),
            ) {
                (Some(t), None) => t.to_string(),
                (None, Some(id)) => drawer_content(&conn, id)?
                    .ok_or_else(|| anyhow::anyhow!("drawer id not found"))?,
                (None, None) => anyhow::bail!("provide text or drawer_id"),
                (Some(_), Some(_)) => anyhow::bail!("use only one of text or drawer_id"),
            };
            let n = extract_to_kg(&conn, &config.llm, &body)?;
            Ok(json!({"kg_facts_added": n}))
        }
        _ => Err(anyhow::anyhow!("unknown tool: {name}")),
    }
}
