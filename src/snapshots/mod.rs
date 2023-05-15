
pub mod program;

use std::collections::BTreeMap;

use tiberius::time::chrono::NaiveDateTime;

pub trait DbSnapshot
{
    /// Enum that tracks what type of a change happend
    type ChangeType;
    /// Connection type for the source database
    type SrcConnection;
    /// Connection type for the tracking database
    type TrackingConnection;

    /// get identifying key for snapshot
    fn get_id(&self) -> &u32;

    /// records mutation in tracking database
    fn record(&self, conn: &Self::TrackingConnection) -> anyhow::Result<()>;
    /// gets mutation from tracking database
    fn get_latest(conn: &Self::TrackingConnection, id: u32) -> anyhow::Result<Option<Self>> where Self: Sized;
    /// gets latest data from source database
    async fn get_src_data<S: AsRef<str>>(conn: &mut Self::SrcConnection, id: S) -> anyhow::Result<Self> where Self: Sized;

    /// to get the change type when Self is removed from source database
    async fn not_found_in_src_change(&self, conn: &mut Self::SrcConnection) -> anyhow::Result<Self::ChangeType>;

    fn calculate_changes(&self, latest: Self) -> Option<Vec<Self::ChangeType>>;
    async fn calculate_snapshot(&self, conn: &mut Self::SrcConnection, desc: impl ToString) -> anyhow::Result<Option<Snapshot<Self::ChangeType>>>
        where Self: Sized
    {
        let latest = Self::get_src_data(conn, self.get_id().to_string()).await;
        let snapshot = if let Err(_) = latest {
            Some(Snapshot {
                timestamp: crate::db::current_time(),
                description: desc.to_string(),
                changes: vec![self.not_found_in_src_change(conn).await?]
            })
        }

        else if let Some(changes) = self.calculate_changes( latest? ) {
            Some(Snapshot {
                timestamp: crate::db::current_time(),
                description: desc.to_string(),
                changes
            })
        }
        else { None };

        Ok(snapshot)
    }
}

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
