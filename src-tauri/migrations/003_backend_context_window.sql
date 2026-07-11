ALTER TABLE llm_backends ADD COLUMN context_window_tokens INTEGER NOT NULL DEFAULT 12000;
