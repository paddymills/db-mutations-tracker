
mod sheet;
mod part;
mod posted;
mod program;

use std::matches;

use chrono::NaiveDateTime;
pub use sheet::Sheet;
pub use part::Part;
pub use program::Program;
pub use posted::PostedProgram;

#[derive(Debug, Serialize, Deserialize)]
pub enum ProgramStatus {
    Posted(NaiveDateTime),
    Deleted(NaiveDateTime),
    Updated(NaiveDateTime)
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PostingChange {
    Posted(NaiveDateTime),
    Deleted(NaiveDateTime),
    Completed(NaiveDateTime),
    NewSheet(Sheet),
    UpdatedSheetData(Sheet),
    AddPart(Part),
    ChangePartQty(u32),
    DeletePart(Part),
}

impl From<ProgramStatus> for PostingChange {
    fn from(value: ProgramStatus) -> Self {
        use ProgramStatus::*;

        match value {
            Posted(ts)  => Self::Posted(ts),
            Deleted(ts) => Self::Deleted(ts),
            Updated(ts) => Self::Completed(ts)
        }
    }
}

impl PartialEq<ProgramStatus> for PostingChange {
    fn eq(&self, other: &ProgramStatus) -> bool {
        matches!( (self, other), (PostingChange::Posted(_),    ProgramStatus::Posted(_))  ) ||
        matches!( (self, other), (PostingChange::Deleted(_),   ProgramStatus::Deleted(_)) ) ||
        matches!( (self, other), (PostingChange::Completed(_), ProgramStatus::Updated(_)) )
    }
}
