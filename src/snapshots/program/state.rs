
use std::collections::HashSet;

use super::{*, tracking::get_db};
use crate::db::DbClient;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgramStateSnapshot {
    pub name: String,
    pub machine: String,
    pub sheet: Sheet,
    pub parts: HashSet<Part>,
    pub status: ProgramStatus,
}

impl ProgramStateSnapshot {
    pub async fn from_src_data<S: AsRef<str>>(conn: &mut DbClient, id: S) -> tiberius::Result<Self> {
        // Query the programs table for the program record with the specified ID
        let program_row = conn
            .query(
                "
                    SELECT
                        Program.ProgramName  AS program,
                        Program.MachineName  AS machine,
                        Program.PostDateTime AS timestamp,

                        Stock.SheetName      AS sheet,
                        Stock.Material       AS grade,
                        Stock.PrimeCode      AS material,
                        Stock.HeatNumber     AS heat,
                        Stock.BinNumber      AS po
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

        Ok( Self::from(program_row) )
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
            ", &[&self.name.to_string()]
        ).await?
        .into_first_result().await?
        .into_iter();

        while let Some(part_row) = part_rows.next() {
            self.parts.insert(Part::from(part_row));
        }

        Ok(())
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
                ", &[&self.name.to_string()]
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
            transtype => Err( anyhow!("Unexpected TransType `{}` for program `{}`", transtype, self.name) )
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
                    { PostingChange::UpdatedSheetData(self.sheet.diff_sheet(latest.sheet)) }
                else
                    { PostingChange::SwapSheet(latest.sheet) }
            );
        }

        // check parts in current snapshot to latest
        for part in self.parts.iter() {
            match latest.parts.take(&part) {
                Some(latest_part) => {
                    if part.qty != latest_part.qty {
                        changes.push( PostingChange::ChangePartQty(latest_part) );
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

    pub async fn get_all_tracked_programs() -> surrealdb::Result<Vec<Self>> {
        get_db().await?
            .select(TRACKING_TABLE).await
    }
}

impl From<tiberius::Row> for ProgramStateSnapshot {
    fn from(row: tiberius::Row) -> Self {
        ProgramStateSnapshot {
            name:        row.get::<&str, _>("program" ).unwrap().into(),
            machine:     row.get::<&str, _>("machine" ).unwrap().into(),
            sheet: Sheet {
                name:    row.get::<&str, _>("sheet"   ).unwrap().into(),
                grade:   row.get::<&str, _>("grade"   ).unwrap().into(),
                mm:      row.get::<&str, _>("material").unwrap().into(),
                heat:    row.get::<&str, _>("heat"    ).unwrap().into(),
                po:      row.get::<&str, _>("po"      ).unwrap().parse()
                    .expect(&format!("Failed to parse PO number: {}", row.get::<&str, _>("po").unwrap())),
            },
            parts: HashSet::new(),
            status: ProgramStatus::Posted(row.get("timestamp").unwrap())
        }
    }
}

impl PartialEq for ProgramStateSnapshot {
    fn eq(&self, other: &Self) -> bool {
        // program numbers match
        self.name == other.name &&

        // sheets match
        self.sheet == other.sheet &&

        // part lists match
        self.parts == other.parts
    }
}

#[cfg(test)]
pub mod test_programstatesnap {
    use crate::{
        snapshots::program::{Sheet, ProgramStatus, Part},
        db::current_time
    };

    use super::ProgramStateSnapshot;
    use std::collections::HashSet;

    #[test]
    fn test_eq() {
        let s1 = Sheet { name: "S12345".into(), grade: "50/50W".into(), mm: "50/50W-0100".into(), heat: "A4A100".into(), po: 4500252867 };
        let s2 = Sheet { name: "X18053".into(), grade: "A709-50T2".into(), mm: "1xx0xxxA-07001".into(), heat: "D6001".into(), po: 4500252867 };
        let s3 = Sheet { name: "X18053".into(), grade: "A709-50T2".into(), mm: "1xx0xxxA-07001".into(), heat: "D6001".into(), po: 4500252867 };

        let p1 = HashSet::from([Part { name: "x1a".into(), wo: "test".into(), qty: 1 }, Part { name: "x1b".into(), wo: "test".into(), qty: 1 }]);
        let p2 = HashSet::from([Part { name: "x1a".into(), wo: "test".into(), qty: 3 }, Part { name: "x1b".into(), wo: "test".into(), qty: 1 }]);
        let p3 = HashSet::from([Part { name: "x1a".into(), wo: "test".into(), qty: 1 }, Part { name: "x1b".into(), wo: "test".into(), qty: 1 }]);

        let a = ProgramStateSnapshot { name: "name".into(), machine: "mach1".into(), sheet: s1, parts: p1, status: ProgramStatus::Posted(current_time()) };
        let b = ProgramStateSnapshot { name: "name".into(), machine: "mach1".into(), sheet: s2, parts: p2, status: ProgramStatus::Posted(current_time()) };
        let c = ProgramStateSnapshot { name: "name".into(), machine: "mach1".into(), sheet: s3, parts: p3, status: ProgramStatus::Posted(current_time()) };

        assert_eq!(b, c);
        assert_ne!(a, b);
    }
}
