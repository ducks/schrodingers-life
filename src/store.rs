use std::{path::PathBuf, sync::Mutex};

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use serde::Serialize;

use crate::creature::Creature;

pub struct Store {
    connection: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Life {
    pub id: i64,
    pub seed: String,
    pub creature: Creature,
    pub born_at: DateTime<Utc>,
    pub died_at: Option<DateTime<Utc>>,
    pub peak_observers: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LifeRecord {
    pub id: i64,
    pub species: String,
    pub rarity: String,
    pub shiny: bool,
    pub born_at: String,
    pub died_at: String,
    pub peak_observers: usize,
    pub duration_seconds: u64,
}

impl Store {
    pub fn open(path: PathBuf) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS lives (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               seed TEXT NOT NULL,
               species TEXT NOT NULL,
               rarity TEXT NOT NULL,
               shiny INTEGER NOT NULL,
               born_at TEXT NOT NULL,
               died_at TEXT,
               peak_observers INTEGER NOT NULL DEFAULT 0
             );",
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn reconcile_unfinished_lives(&self) -> Result<usize> {
        let died_at = Utc::now().to_rfc3339();
        let updated = self
            .connection
            .lock()
            .expect("database lock poisoned")
            .execute(
                "UPDATE lives SET died_at = ?1 WHERE died_at IS NULL",
                [died_at],
            )?;
        Ok(updated)
    }

    pub fn begin_life(&self, seed: String, creature: Creature) -> Result<Life> {
        let born_at = Utc::now();
        let connection = self.connection.lock().expect("database lock poisoned");
        connection.execute(
            "INSERT INTO lives (seed, species, rarity, shiny, born_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                seed,
                creature.species,
                creature.rarity.name(),
                creature.shiny,
                born_at.to_rfc3339(),
            ],
        )?;

        Ok(Life {
            id: connection.last_insert_rowid(),
            seed,
            creature,
            born_at,
            died_at: None,
            peak_observers: 0,
        })
    }

    pub fn end_life(&self, life: &Life) -> Result<()> {
        let died_at = life.died_at.expect("dead life has a death time");
        self.connection
            .lock()
            .expect("database lock poisoned")
            .execute(
                "UPDATE lives SET died_at = ?1, peak_observers = ?2 WHERE id = ?3",
                params![died_at.to_rfc3339(), life.peak_observers, life.id],
            )?;
        Ok(())
    }

    pub fn history(&self, limit: usize) -> Result<Vec<LifeRecord>> {
        let connection = self.connection.lock().expect("database lock poisoned");
        let mut statement = connection.prepare(
            "SELECT id, species, rarity, shiny, born_at, died_at, peak_observers,
                    MAX(0, CAST(ROUND((julianday(died_at) - julianday(born_at)) * 86400) AS INTEGER))
             FROM lives WHERE died_at IS NOT NULL ORDER BY id DESC LIMIT ?1",
        )?;
        let records = statement
            .query_map([limit], life_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(records)
    }

    pub fn longest_life(&self) -> Result<Option<LifeRecord>> {
        let connection = self.connection.lock().expect("database lock poisoned");
        let mut statement = connection.prepare(
            "SELECT id, species, rarity, shiny, born_at, died_at, peak_observers,
                    MAX(0, CAST(ROUND((julianday(died_at) - julianday(born_at)) * 86400) AS INTEGER))
             FROM lives
             WHERE died_at IS NOT NULL
             ORDER BY (julianday(died_at) - julianday(born_at)) DESC, id ASC
             LIMIT 1",
        )?;
        let mut records = statement.query_map([], life_record)?;
        Ok(records.next().transpose()?)
    }
}

fn life_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<LifeRecord> {
    Ok(LifeRecord {
        id: row.get(0)?,
        species: row.get(1)?,
        rarity: row.get(2)?,
        shiny: row.get(3)?,
        born_at: row.get(4)?,
        died_at: row.get(5)?,
        peak_observers: row.get(6)?,
        duration_seconds: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creature;

    #[test]
    fn unfinished_lives_are_closed_during_reconciliation() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("lives.db");
        let store = Store::open(database).unwrap();
        let seed = "interrupted-life".to_owned();
        store
            .begin_life(seed.clone(), creature::roll(&seed))
            .unwrap();

        assert_eq!(store.reconcile_unfinished_lives().unwrap(), 1);
        let history = store.history(20).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, 1);
    }

    #[test]
    fn longest_life_returns_the_duration_record() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(directory.path().join("lives.db")).unwrap();
        let connection = store.connection.lock().unwrap();
        connection
            .execute(
                "INSERT INTO lives
                 (seed, species, rarity, shiny, born_at, died_at, peak_observers)
                 VALUES
                 ('short', 'cat', 'common', 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:10Z', 1),
                 ('record', 'duck', 'rare', 0, '2026-01-01T00:00:00Z', '2026-01-01T00:02:00Z', 8)",
                [],
            )
            .unwrap();
        drop(connection);

        let record = store.longest_life().unwrap().unwrap();
        assert_eq!(record.species, "duck");
        assert_eq!(record.duration_seconds, 120);
        assert_eq!(record.peak_observers, 8);
    }
}
