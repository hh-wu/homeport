CREATE TABLE ai_memory (
  key        TEXT PRIMARY KEY NOT NULL,
  value      TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_ai_memory_updated ON ai_memory (updated_at);
