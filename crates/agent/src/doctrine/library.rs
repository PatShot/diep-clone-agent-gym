//! A set of doctrines a team is seated with, so a control center can switch a
//! tank between them by name.
//!
//! The library is the control center's menu. `SetDoctrine` names an entry; a
//! name not on the menu is refused and counted, never guessed at. Shared by every
//! tank on a team behind an `Arc`, because a doctrine is read and never written
//! once loaded.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use schema::{AgentId, Role, TeamId};

use super::engine::DoctrinePolicy;
use super::{Doctrine, DoctrineError};
use crate::runner::Runner;

#[derive(Debug, Default)]
pub struct DoctrineLibrary {
    doctrines: BTreeMap<String, Arc<Doctrine>>,
}

impl DoctrineLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// A library of one.
    pub fn of(doctrine: Doctrine) -> Self {
        let mut lib = Self::new();
        lib.insert(doctrine);
        lib
    }

    /// Add a doctrine under its own name, replacing any of that name.
    pub fn insert(&mut self, doctrine: Doctrine) -> &mut Self {
        self.doctrines
            .insert(doctrine.name.clone(), Arc::new(doctrine));
        self
    }

    /// Every `.toml` and `.json` in a directory, validated. Order of loading
    /// does not matter; names do, and a duplicate name is the later file.
    pub fn from_dir(dir: impl AsRef<Path>) -> Result<Self, DoctrineError> {
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .map_err(DoctrineError::Io)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "toml" || e == "json"))
            .collect();
        paths.sort();
        let mut lib = Self::new();
        for p in paths {
            lib.insert(Doctrine::from_path(&p)?);
        }
        Ok(lib)
    }

    pub fn get(&self, name: &str) -> Option<&Arc<Doctrine>> {
        self.doctrines.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.doctrines.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.doctrines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.doctrines.is_empty()
    }

    /// Seat every tank of `team` on the doctrine named `initial`, with this whole
    /// library available to `SetDoctrine`. Returns who got which role.
    pub fn seat(
        self: &Arc<Self>,
        runner: &mut Runner,
        team: TeamId,
        initial: &str,
    ) -> Result<Vec<(AgentId, Role)>, DoctrineError> {
        let doctrine = self
            .get(initial)
            .ok_or_else(|| DoctrineError::UnknownDoctrine(initial.to_string()))?
            .clone();
        let seats: Vec<AgentId> = runner
            .seats()
            .filter(|(_, t, is_cc)| *t == team && !is_cc)
            .map(|(a, _, _)| a)
            .collect();
        let arena = runner.world().arena.clone();
        let seed = runner.world().spec().seed;
        let assigned = doctrine.assign(&seats);
        for (agent, role) in &assigned {
            let policy = DoctrinePolicy::with_library(
                self.clone(),
                initial,
                *role,
                team,
                &arena,
                seed,
                *agent,
            )?;
            runner.set_policy(*agent, Box::new(policy));
        }
        Ok(assigned)
    }
}
