# Ingestion-to-feed E2E harness

The Rust harness uses an isolated PostgreSQL database supplied through
`E2E_DATABASE_URL`. It starts local fixture HTTP routes for both the policy
source and the Claude-compatible summarizer, then exercises the real fetcher,
normalizer, change detector, and feed handler.

Run it with:

```bash
E2E_DATABASE_URL=postgresql://... \
  cargo test -p policy-backend fixture_source_flows_from_ingestion_to_feed -- --ignored
```

The fixture-backed frontend rendering check is run with:

```bash
cd frontend
npm run test:e2e
```
