
use std::collections::HashSet;

use serde::Serialize;
use surrealdb::engine::local::Mem;

use crate::db::DbPool;
use super::DbSnapshot;

#[derive(Debug, Default, Serialize)]
pub struct PostedProgram {
    pub program: Program,
    pub sheet: Sheet,
    pub parts: HashSet<Part>
}

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct Program {
    pub name: u32,
    pub machine: String,
}

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct Sheet {
    pub name: String,
    pub grade: String,
    pub mm: String,
    pub heat: String,
    pub po: String
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Part {
    pub name: String,
    pub wo: String,
    pub qty: u32
}

#[derive(Debug, Serialize)]
pub enum PostingChange {
    Posted,
    Deleted,
    Completed,
    NewSheet(Sheet),
    UpdatedSheetData(Sheet),
    AddPart(Part),
    ChangePartQty(u32),
    DeletePart(Part),
}

impl PostedProgram {
    pub async fn get_all_active_programs(pool: &mut DbPool) -> anyhow::Result<Vec<Self>> {
        let tasks = pool.get().await?
            .simple_query( "SELECT ProgramName FROM Program" )
                .await?
            .into_first_result()
                .await?
            .into_iter()
            .map(|row| {
                let pool = pool.clone();

                tokio::spawn(async move {
                    Self::get_src_data(
                        &mut pool.get().await.unwrap(),
                        &row.get::<&str, _>("ProgramName").unwrap()
                    )
                        .await
                        .unwrap()
                })
            });

        Ok(
            futures::future::try_join_all(
                tasks
            ).await?
        )
    }

    pub fn get_all_tracked_programs() -> anyhow::Result<Vec<PostedProgram>> {
        // TODO: get all from SurrealDB

        Ok(vec![])
    }

    /// takes a snapshot of all programs in both databases
    pub async fn take_database_snapshot(pool: &mut DbPool) -> anyhow::Result<()> {
        // TODO: migrate to SurrealDB
        let _tracking_db = super::TRACKING_DB.connect::<Mem>(()).await?;

        let _active_programs = Self::get_all_active_programs(pool).await?;
        // let tracked_programs = Self::

        Ok(())
    }
}

impl DbSnapshot for PostedProgram {
    type ChangeType = PostingChange;
    type SrcConnection = crate::db::DbClient;

    fn get_id(&self) -> &u32 {
        &self.program.name
    }

    fn record(&self) -> anyhow::Result<()> {
        // TODO: impl
        Ok(())
    }

    fn get_latest(id: u32) -> anyhow::Result<Option<Self>> where Self: Sized {
        // TODO: impl
    
        Ok(Some(Self::default()))
    }

    async fn get_src_data<S: AsRef<str>>(conn: &mut Self::SrcConnection, id: S) -> anyhow::Result<Self> where Self: Sized {
        // Query the programs table for the program record with the specified ID
        let program_row = conn
            .query(
                "
                    SELECT
                        Program.ProgramName AS program,
                        Program.MachineName AS machine,
                        Stock.SheetName     AS sheet,
                        Stock.Material      AS grade,
                        Stock.PrimeCode     AS material,
                        Stock.HeatNumber    AS heat,
                        Stock.BinNumber     AS po
                    FROM Program
                    INNER JOIN Stock
                        ON Program.SheetName = Stock.SheetName
                    WHERE Program.ProgramName = @P1
                ", &[&id.as_ref()]
            )
            .await?
            .into_row()
            .await?
            .ok_or_else(|| tiberius::error::Error::Protocol("no program found".into()))?;

        let mut posted = PostedProgram {
            program: Program {
                name:    program_row.get::<&str, _>("program" ).unwrap().parse()?,
                machine: program_row.get::<&str, _>("machine" ).unwrap().into(),
            },
            sheet: Sheet {
                name:    program_row.get::<&str, _>("sheet"   ).unwrap().into(),
                grade:   program_row.get::<&str, _>("grade"   ).unwrap().into(),
                mm:      program_row.get::<&str, _>("material").unwrap().into(),
                heat:    program_row.get::<&str, _>("heat"    ).unwrap().into(),
                po:      program_row.get::<&str, _>("po"      ).unwrap().into(),
            },
            parts: HashSet::new()
        };

        // Query the parts table for all part records associated with the program ID
        let mut part_rows = conn.query(
            "
                SELECT
                    PartName     AS part,
                    WONumber     AS workorder,
                    QtyInProcess AS qty
                FROM PIP
                WHERE ProgramName = @P1
            ", &[&posted.program.name.to_string()]
        ).await?
        .into_first_result().await?
        .into_iter();

        while let Some(part_row) = part_rows.next() {
            posted.parts.insert(Part {
                name: part_row.get::<&str, _>("name").unwrap().into(),
                wo:   part_row.get::<&str, _>("workorder").unwrap().into(),
                qty:  part_row.get::<i32,  _>("qty" ).unwrap().unsigned_abs(),
            });
        }

        Ok( posted )
    }

    async fn not_found_in_src_change(&self, conn: &mut Self::SrcConnection) -> anyhow::Result<Self::ChangeType> {
        let program_row = conn
            .query(
                "
                    SELECT TransType
                    FROM ProgArchive
                    WHERE ProgramName = @P1
                ", &[&self.get_id().to_string()]
            )
            .await?
            .into_row()
            .await?
            .ok_or_else(|| tiberius::error::Error::Protocol("no program found".into()))?;

        match program_row.get("TransType").unwrap() {
            "SN101"   => Ok( PostingChange::Deleted   ),
            "SN102"   => Ok( PostingChange::Completed ),
            transtype => Err( anyhow!("Unexpected TransType `{}` for program `{}`", transtype, self.get_id()) )
        }
    }

    fn calculate_changes(&self, mut latest: PostedProgram) -> Option<Vec<Self::ChangeType>> {
        if self == &latest {
            return None;
        }

        let mut changes = Vec::new();
        
        // check if sheets are the same
        if self.sheet != latest.sheet {
            changes.push(
                if self.sheet.name == latest.sheet.name
                    { PostingChange::UpdatedSheetData(latest.sheet) }
                else
                    { PostingChange::NewSheet(latest.sheet) }
            );
        }

        // check parts in current snapshot to latest
        for part in self.parts.iter() {
            match latest.parts.take(&part) {
                Some(latest_part) => {
                    if part.qty != latest_part.qty {
                        changes.push( PostingChange::ChangePartQty(latest_part.qty) );
                    }
                    // else: parts are the samed and part is essentially consumed from latest.parts
                    //       this is useful for adding any unconsumed parts to changes below
                },
                None => changes.push( PostingChange::DeletePart(part.to_owned()) )
            }
        }

        // parts only in latest
        changes.append(
            &mut latest.parts
                .into_iter()
                .map(|part| PostingChange::AddPart(part))
                .collect()
        );

        Some(changes)
    }
}

impl PartialEq for PostedProgram {
    fn eq(&self, other: &Self) -> bool {
        // program numbers match
        self.program == other.program &&

        // sheets match
        self.sheet == other.sheet &&

        // symmetric_difference returns values
        //  in self.parts or other.parts, but not both
        // should be an empty iterator if self.parts == other.parts
        self.parts.symmetric_difference(&other.parts).count() == 0
    }
}
