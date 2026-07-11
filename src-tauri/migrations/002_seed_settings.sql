INSERT OR IGNORE INTO settings(key, value) VALUES
  ('spill.max_file_size_mb', '10'),
  ('spill.global_budget_mb', '5000'),
  ('spill.retention_days', '30'),
  ('spill.compress_after_days', '7'),
  ('agent.autorun_timeout_seconds', '30'),
  ('settings.git_commit_tier', 'confirm'),
  ('appearance.font_family', 'Menlo'),
  ('appearance.font_size', '13'),
  ('agent.panel_width', '360');
