
use std::collections::HashSet;

use rusqlite::named_params;

use crate::db::{self, current_time, DbPool};
use super::DbSnapshot;

const SNDB_TRACKING_DB: &str = "sndb_mutations";

#[derive(Debug)]
pub struct PostedProgram {
    pub program: Program,
    pub sheet: Sheet,
    pub parts: HashSet<Part>
}

#[derive(Debug, PartialEq)]
pub struct Program {
    pub name: u32,
    pub machine: String,
}

#[derive(Debug, PartialEq)]
pub struct Sheet {
    pub name: String,
    pub grade: String,
    pub mm: String,
    pub heat: String,
    pub po: String
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Part {
    pub name: String,
    pub wo: String,
    pub qty: u32
}

#[derive(Debug)]
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
    pub async fn get_all_programs(pool: &mut DbPool) -> anyhow::Result<Vec<Self>> {
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

    pub fn get_all_tracked_programs(conn: rusqlite::Connection) -> anyhow::Result<Vec<PostedProgram>> {
        let mut stmt = conn.prepare("
            SELECT snapshot
            FROM snapshots
        ")?;
        let ids = stmt.query_map([], |row| row.get(0))?;

        Ok(vec![])
    }

    /// takes a snapshot of all programs in both databases
    pub async fn take_database_snapshot(pool: &mut DbPool) -> anyhow::Result<()> {
        // TODO: migrate to SurrealDB
        let _tracking_db = rusqlite::Connection::open( SNDB_TRACKING_DB );

        let _active_programs = Self::get_all_programs(pool).await?;
        // let tracked_programs = Self::

        Ok(())
    }
}

impl DbSnapshot for PostedProgram {
    type ChangeType = PostingChange;
    type TrackingConnection = rusqlite::Connection;
    type SrcConnection = crate::db::DbClient;

    fn get_id(&self) -> &u32 {
        &self.program.name
    }

    fn record(&self, conn: &Self::TrackingConnection) -> anyhow::Result<()> {
        // Insert the program record into the programs table
        let mut stmt = conn.prepare("
            INSERT INTO snapshots (program, timestamp)
            VALUES (:name, :timestamp)
        ")?;
        stmt.execute(named_params! {
            ":name": self.program.name,
            ":timestamp": current_time().timestamp()
        })?;
    
        // Get the ID of the newly inserted program record
        let program_id = conn.last_insert_rowid();
    
        // Insert the part records into the parts table, using the program ID as a foreign key
        let mut stmt = conn.prepare("
            INSERT INTO parts (program_id, name, qty)
            VALUES (:id, :part, :qty)
        ")?;
        for part in &self.parts {
            stmt.execute(named_params! {
                ":id": program_id,
                ":part": part.name,
                ":qty": part.qty
            })?;
        }
    
        Ok(())
    }

    fn get_latest(conn: &Self::TrackingConnection, id: u32) -> anyhow::Result<Option<Self>> where Self: Sized {
        // Query the programs table for the program record with the specified ID
        let mut stmt = conn.prepare("
            SELECT TOP 1
                program,
                sheet_name,
                sheet_grade,
                sheet_mm,
                sheet_heat,
                sheet_po,
                machine
            FROM programs
            WHERE program = :id
            ORDER BY snapshot DESC
        ")?;
        let program_row = stmt.query_row(&[(":id", &id)],
        |row| {
            Ok(Self {
                program: Program {
                    machine: row.get("machine")?,
                    name: row.get("program")?,
                },
                sheet: Sheet {
                    name: row.get("sheet_name")?,
                    grade: row.get("sheet_grade")?,
                    mm: row.get("sheet_mm")?,
                    heat: row.get("sheet_heat")?,
                    po: row.get("sheet_po")?
                },
                parts: HashSet::new()   // temporary, filled below
            })
        });
    
        // If no program record is found with the specified ID, return None
        let mut program = match program_row {
            Ok(program) => program,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(err) => return Err(err.into())
        };
    
        // Query the parts table for all part records associated with the program ID
        let mut stmt = conn.prepare("
            SELECT
                name, workorder, qty
            FROM parts
            WHERE program_id = :id
        ")?;
        let part_rows = stmt.query_map(&[(":id", &id)],
        |row| {
            Ok(Part {
                name: row.get("name")?,
                wo: row.get("workorder")?,
                qty: row.get("qty")?
            })
        })?;
    
        // Add the part records to the program struct
        program.parts = HashSet::from_iter( part_rows.map( |row| row.unwrap() ) );
    
        Ok(Some(program))
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
