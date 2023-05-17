
mod lifetime;
mod part;
mod state;
mod sheet;

pub use lifetime::ProgramHistory;
pub use part::Part;
pub use state::ProgramStateSnapshot;
pub use sheet::{Sheet, SheetData};
pub use super::tracking;

use crate::db::DbPool;
use std::{matches, collections::HashSet};
use chrono::NaiveDateTime;

pub static TRACKING_TABLE: &str = "program";

#[derive(Debug, Serialize, Deserialize)]
pub enum ProgramStatus {
    Posted(NaiveDateTime),
    Deleted(NaiveDateTime),
    Updated(NaiveDateTime)
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PostingChange {
    Posted {
        timestamp: NaiveDateTime,
        machine: String, // TODO: machine enum
        sheet: Sheet,
        parts: HashSet<Part>
    },
    Deleted(NaiveDateTime),
    Completed(NaiveDateTime),   // TODO: add operator
    ChangeMachine(String),
    SwapSheet(Sheet),
    UpdatedSheetData(Vec<SheetData>),    // TODO: sheet update enum
    AddPart(Part),
    ChangePartQty(Part),
    DeletePart(Part),
}

impl From<ProgramStatus> for PostingChange {
    fn from(value: ProgramStatus) -> Self {
        use ProgramStatus::*;

        match value {
            Posted(_)  => panic!("Cannot convert ProgramStatus::Posted to PostingChange. Additional information is needed"),
            Deleted(ts) => Self::Deleted(ts),
            Updated(ts) => Self::Completed(ts)
        }
    }
}

impl PartialEq<ProgramStatus> for PostingChange {
    fn eq(&self, other: &ProgramStatus) -> bool {
        matches!( (self, other), (PostingChange::Posted {..},  ProgramStatus::Posted(_))  ) ||
        matches!( (self, other), (PostingChange::Deleted(_),   ProgramStatus::Deleted(_)) ) ||
        matches!( (self, other), (PostingChange::Completed(_), ProgramStatus::Updated(_)) )
    }
}

pub async fn get_all_active_programs(pool: &mut DbPool) -> anyhow::Result<Vec<ProgramStateSnapshot>> {
    let tasks = pool.get().await?
        .simple_query( "SELECT ProgramName FROM Program" )
            .await?
        .into_first_result()
            .await?
        .into_iter()
        .map(|row| {
            let pool = pool.clone();

            tokio::spawn(async move {
                ProgramStateSnapshot::from_src_data(
                    &mut pool.get().await.unwrap(),
                    &row.get::<&str, _>("ProgramName").unwrap()
                )
                    .await
                    .unwrap()
            })
        });

    Ok( futures::future::try_join_all( tasks ).await? )
}

/// takes a snapshot of all programs in both databases
pub async fn take_database_snapshot(pool: &mut DbPool) -> anyhow::Result<()> {
    let _tracking_db = tracking::get_db().await?;

    let _active_programs = get_all_active_programs(pool).await?;
    // let tracked_programs = Self::

    Ok(())
}
