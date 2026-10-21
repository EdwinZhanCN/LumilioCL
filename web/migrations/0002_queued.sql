-- Summaries handed to the translation queue (worker/translate.ts, plan
-- translation-fallback D6). A row holds its platform, ID and hash for
-- CLAIM_SECONDS, so requests that arrive while a job runs queue it only once;
-- a saved translation removes it, and an expired one is cleared at the next
-- claim.
CREATE TABLE queued (
  platform TEXT NOT NULL,
  id TEXT NOT NULL,
  hash TEXT NOT NULL,
  claimed INTEGER NOT NULL,
  PRIMARY KEY (platform, id, hash)
);
