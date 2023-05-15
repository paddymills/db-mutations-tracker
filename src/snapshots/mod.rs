
pub mod program;
pub mod tracking;

use std::collections::BTreeMap;
use tiberius::time::chrono::NaiveDateTime;

#[derive(Debug)]
pub struct Timeline<Change> {
    pub snapshots: BTreeMap<i64, Snapshot<Change>>
}

#[derive(Debug)]
pub struct Snapshot<Change> {
    pub timestamp:   NaiveDateTime,
    pub description: String,
    pub changes:     Vec<Change>,
}

impl<C> Timeline<C> {
    pub fn add_snapshot(&mut self, snapshot: Snapshot<C>) -> anyhow::Result<()> {
        let _ = self.snapshots.insert(snapshot.timestamp.timestamp(), snapshot);

        Ok(())
    }
}
