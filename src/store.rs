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
            "SELECT id, species, rarity, shiny, born_at, died_at, peak_observers
             FROM lives WHERE died_at IS NOT NULL ORDER BY id DESC LIMIT ?1",
        )?;
        let records = statement
            .query_map([limit], |row| {
                Ok(LifeRecord {
                    id: row.get(0)?,
                    species: row.get(1)?,
                    rarity: row.get(2)?,
                    shiny: row.get(3)?,
                    born_at: row.get(4)?,
                    died_at: row.get(5)?,
                    peak_observers: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(records)
    }
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
}
