
use std::collections::HashSet;

use super::*;
use crate::db::{DbClient, DbPool};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PostedProgram {
    pub program: Program,
    pub sheet: Sheet,
    pub parts: HashSet<Part>
}

impl PostedProgram {
    pub async fn get_src_data<S: AsRef<str>>(conn: &mut DbClient, id: S) -> tiberius::Result<Self> {
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

        Ok( PostedProgram::from(program_row) )
    }

    pub async fn get_parts(&mut self, conn: &mut DbClient) -> tiberius::Result<()> {
        // Query the parts table for all part records associated with the program ID
        let mut part_rows = conn.query(
            "
                SELECT
                    PartName     AS part,
                    WONumber     AS workorder,
                    QtyInProcess AS qty
                FROM PIP
                WHERE ProgramName = @P1
            ", &[&self.program.name.to_string()]
        ).await?
        .into_first_result().await?
        .into_iter();

        while let Some(part_row) = part_rows.next() {
            self.parts.insert(Part {
                name: part_row.get::<&str, _>("name").unwrap().into(),
                wo:   part_row.get::<&str, _>("workorder").unwrap().into(),
                qty:  part_row.get::<i32,  _>("qty" ).unwrap().unsigned_abs(),
            });
        }

        Ok(())
    }

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

    pub async fn is_updated_or_deleted(&self, conn: &mut DbClient) -> anyhow::Result<ProgramStatus> {
        let program_row = conn
            .query(
                "
                    SELECT
                        ArcDateTime AS timestamp,
                        TransType   AS tcode
                    FROM ProgArchive
                    WHERE ProgramName = @P1
                ", &[&self.program.name.to_string()]
            )
            .await?
            .into_row()
            .await?
            .ok_or_else(|| tiberius::error::Error::Protocol("no program found".into()))?;

        let timestamp = program_row.get("timestamp").unwrap();
        match program_row.get("tcode").unwrap() {
            "SN100"   => Ok( ProgramStatus::Posted (timestamp) ),
            "SN101"   => Ok( ProgramStatus::Deleted(timestamp) ),
            "SN102"   => Ok( ProgramStatus::Updated(timestamp) ),
            transtype => Err( anyhow!("Unexpected TransType `{}` for program `{}`", transtype, self.program.name) )
        }
    }

    pub fn calculate_changes(&self, mut latest: Self) -> Option<Vec<PostingChange>> {
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

    pub fn get_all_tracked_programs() -> anyhow::Result<Vec<PostedProgram>> {
        // TODO: get all from SurrealDB

        Ok(vec![])
    }

    /// takes a snapshot of all programs in both databases
    pub async fn take_database_snapshot(pool: &mut DbPool) -> anyhow::Result<()> {
        let _tracking_db = super::super::tracking::get_db().await?;

        let _active_programs = Self::get_all_active_programs(pool).await?;
        // let tracked_programs = Self::

        Ok(())
    }
}

impl From<tiberius::Row> for PostedProgram {
    fn from(row: tiberius::Row) -> Self {
        let name = row.get::<&str, _>("program" ).unwrap();

        PostedProgram {
            program: Program {
                name:    name.parse().expect(&format!("Failed to parse program name; `{}`", name)),
                machine: row.get::<&str, _>("machine" ).unwrap().into(),
            },
            sheet: Sheet {
                name:    row.get::<&str, _>("sheet"   ).unwrap().into(),
                grade:   row.get::<&str, _>("grade"   ).unwrap().into(),
                mm:      row.get::<&str, _>("material").unwrap().into(),
                heat:    row.get::<&str, _>("heat"    ).unwrap().into(),
                po:      row.get::<&str, _>("po"      ).unwrap().into(),
            },
            parts: HashSet::new()
        }
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
