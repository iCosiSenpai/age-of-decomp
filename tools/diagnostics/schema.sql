CREATE TABLE IF NOT EXISTS reports (
  day TEXT NOT NULL,
  version TEXT NOT NULL,
  os TEXT NOT NULL,
  event TEXT NOT NULL,
  detail TEXT NOT NULL,
  fps INTEGER NOT NULL,
  total INTEGER NOT NULL,
  PRIMARY KEY (day, version, os, event, detail, fps)
) WITHOUT ROWID;
