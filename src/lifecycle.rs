use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

use crate::{
    creature,
    store::{Life, LifeRecord, Store},
};

pub struct AppState {
    inner: Mutex<World>,
    store: Store,
    grace: Duration,
    updates: broadcast::Sender<()>,
}

struct World {
    current: Option<Life>,
    observers: HashSet<Uuid>,
    death_deadline: Option<Instant>,
    generation: u64,
}

#[derive(Serialize)]
pub struct Snapshot {
    pub alive: bool,
    pub life: Option<Life>,
    pub observers: usize,
    pub death_in_seconds: Option<u64>,
    pub graveyard: Vec<LifeRecord>,
}

impl AppState {
    pub fn new(store: Store, grace: Duration) -> Result<Self> {
        let reconciled = store.reconcile_unfinished_lives()?;
        if reconciled > 0 {
            tracing::warn!(
                reconciled,
                "closed lives interrupted while the apparatus was offline"
            );
        }
        let (updates, _) = broadcast::channel(32);
        Ok(Self {
            inner: Mutex::new(World {
                current: None,
                observers: HashSet::new(),
                death_deadline: None,
                generation: 0,
            }),
            store,
            grace,
            updates,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.updates.subscribe()
    }

    pub async fn observe(self: &Arc<Self>) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let mut world = self.inner.lock().await;
        world.generation += 1;
        world.death_deadline = None;

        if world.current.is_none() {
            let seed = format!(
                "life:{}:{}",
                Utc::now().timestamp_nanos_opt().unwrap_or_default(),
                id
            );
            world.current = Some(self.store.begin_life(seed.clone(), creature::roll(&seed))?);
        }

        world.observers.insert(id);
        let observer_count = world.observers.len();
        if let Some(life) = &mut world.current {
            life.peak_observers = life.peak_observers.max(observer_count);
        }
        drop(world);
        let _ = self.updates.send(());
        Ok(id)
    }

    pub async fn stop_observing(self: &Arc<Self>, id: Uuid) {
        let mut world = self.inner.lock().await;
        if !world.observers.remove(&id) || !world.observers.is_empty() {
            drop(world);
            let _ = self.updates.send(());
            return;
        }

        world.generation += 1;
        let generation = world.generation;
        world.death_deadline = Some(Instant::now() + self.grace);
        drop(world);
        let _ = self.updates.send(());

        let state = Arc::clone(self);
        tokio::spawn(async move {
            tokio::time::sleep(state.grace).await;
            state.collapse(generation).await;
        });
    }

    async fn collapse(&self, generation: u64) {
        let mut world = self.inner.lock().await;
        if world.generation != generation || !world.observers.is_empty() {
            return;
        }
        if let Some(mut life) = world.current.take() {
            life.died_at = Some(Utc::now());
            if let Err(error) = self.store.end_life(&life) {
                tracing::error!(%error, life_id = life.id, "could not record death");
                world.current = Some(life);
                return;
            }
        }
        world.death_deadline = None;
        drop(world);
        let _ = self.updates.send(());
    }

    pub async fn snapshot(&self) -> Result<Snapshot> {
        let world = self.inner.lock().await;
        let death_in_seconds = world.death_deadline.map(|deadline| {
            deadline
                .saturating_duration_since(Instant::now())
                .as_secs()
                .saturating_add(1)
        });
        Ok(Snapshot {
            alive: world.current.is_some(),
            life: world.current.clone(),
            observers: world.observers.len(),
            death_in_seconds,
            graveyard: self.store.history(20)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(grace: Duration) -> Arc<AppState> {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("test.db");
        std::mem::forget(directory);
        Arc::new(AppState::new(Store::open(database).unwrap(), grace).unwrap())
    }

    #[tokio::test]
    async fn final_observer_departure_ends_the_life() {
        let state = state(Duration::from_millis(20));
        let observer = state.observe().await.unwrap();

        assert!(state.snapshot().await.unwrap().alive);
        state.stop_observing(observer).await;
        tokio::time::sleep(Duration::from_millis(40)).await;

        let snapshot = state.snapshot().await.unwrap();
        assert!(!snapshot.alive);
        assert_eq!(snapshot.graveyard.len(), 1);
    }

    #[tokio::test]
    async fn returning_observer_cancels_death() {
        let state = state(Duration::from_millis(30));
        let first = state.observe().await.unwrap();
        state.stop_observing(first).await;

        tokio::time::sleep(Duration::from_millis(10)).await;
        let second = state.observe().await.unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;

        let snapshot = state.snapshot().await.unwrap();
        assert!(snapshot.alive);
        assert_eq!(snapshot.observers, 1);
        assert!(snapshot.graveyard.is_empty());
        state.stop_observing(second).await;
    }

    #[tokio::test]
    async fn startup_moves_an_interrupted_life_to_the_graveyard() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("test.db");
        let store = Store::open(database.clone()).unwrap();
        let seed = "life-before-restart".to_owned();
        store
            .begin_life(seed.clone(), creature::roll(&seed))
            .unwrap();
        drop(store);

        let state = AppState::new(Store::open(database).unwrap(), Duration::from_secs(30)).unwrap();
        let snapshot = state.snapshot().await.unwrap();

        assert!(!snapshot.alive);
        assert_eq!(snapshot.graveyard.len(), 1);
    }
}
