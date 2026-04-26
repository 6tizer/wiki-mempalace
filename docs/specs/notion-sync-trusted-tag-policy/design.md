# Design: Notion Sync Trusted Tag Policy

## CLI

Default:

```bash
wiki notion-sync --db-id all
```

Explicit trusted mode:

```bash
wiki notion-sync --db-id x_bookmark --tag-policy trusted-source
```

Conservative/debug mode:

```bash
wiki notion-sync --db-id x_bookmark --tag-policy strict
```

Historical wording remains accepted:

```bash
wiki notion-sync --db-id x_bookmark --tag-policy bootstrap
```

## Behavior

`strict` leaves the loaded schema untouched.

`trusted-source` and `bootstrap` mutate only the in-memory schema for this
process:

```text
max_new_tags_per_ingest = unlimited
deprecated_tags = allow
```

The repo schema file is not rewritten.

## Rationale

Notion AI auto-fill is part of the upstream knowledge workflow. It applies an
A/B/C classification prompt before wiki ingest sees the page. The wiki should
preserve that source metadata first, then use later tag governance to report,
merge, or retire tags.
