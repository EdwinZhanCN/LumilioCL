-- Tables of worker/translate.ts (plan translation-fallback). Apply with
-- `wrangler d1 migrations apply lumilio-translations --remote` before the
-- Worker that reads them is deployed; worker/translate.test.ts runs every
-- file here on its in-memory database.

-- One translated summary per platform, project ID and SHA-256 of the
-- original; a changed summary gets a new row.
CREATE TABLE summaries (
  platform TEXT NOT NULL,
  id TEXT NOT NULL,
  hash TEXT NOT NULL,
  translated TEXT NOT NULL,
  created INTEGER NOT NULL,
  PRIMARY KEY (platform, id, hash)
);

-- Workers AI neurons spent per UTC day (YYYY-MM-DD).
CREATE TABLE budget (
  day TEXT PRIMARY KEY,
  neurons REAL NOT NULL
);
