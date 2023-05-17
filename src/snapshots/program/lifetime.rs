
use std::collections::LinkedList;

use super::tracking::get_db;
use super::{PostingChange, ProgramStateSnapshot, ProgramStatus, TRACKING_TABLE};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgramHistory {
    pub program: String,
    pub changes: LinkedList<PostingChange>
}

impl ProgramHistory {
    pub fn new<S: ToString>(name: S, posting_data: PostingChange) -> Self {
        assert!(matches!(posting_data, PostingChange::Posted { .. }), "Initial history item must be PostingChange::Posted");

        let mut changes = LinkedList::new();
        changes.push_back(posting_data);

        Self { program: name.to_string(), changes }
    }

    pub async fn record(&self) -> surrealdb::Result<()> {
        get_db().await?
            .create((TRACKING_TABLE, &self.program))
            .content(self)
            .await?;

        Ok(())
    }

    pub async fn get_tracked<S: AsRef<str>>(program_number: S) -> surrealdb::Result<Self> {
        get_db().await?
            .select(
                (TRACKING_TABLE, program_number.as_ref())
            ).await
    }

    pub async fn get_current_state<S: AsRef<str> + ToString>(program_number: S) -> surrealdb::Result<ProgramStateSnapshot> {
        let program = Self::get_tracked(&program_number).await?;

        // Traverse linked list to build final state
        let mut history = program.changes.into_iter();
        let mut result = match history.next() {
            Some( PostingChange::Posted { timestamp, machine, sheet, parts } ) => 
            ProgramStateSnapshot { 
                name: program_number.to_string(),
                machine,
                sheet,
                parts,
                status: ProgramStatus::Posted(timestamp),
            },
            _ => panic!("ProgramHistory head must be PostingChange::Posted")
        };

        while let Some(state) = history.next() {
            match state {
                PostingChange::Posted { timestamp, machine, sheet, parts } => {
                    result = ProgramStateSnapshot { 
                        name: result.name,
                        machine,
                        sheet,
                        parts,
                        status: ProgramStatus::Posted(timestamp),
                    }
                },
                PostingChange::Deleted(ts)                  => { result.status  = ProgramStatus::Deleted(ts); },
                PostingChange::Completed(ts)                => { result.status  = ProgramStatus::Updated(ts); },
                PostingChange::ChangeMachine(mach)          => { result.machine = mach; },
                PostingChange::SwapSheet(sheet)             => { result.sheet   = sheet; },
                PostingChange::UpdatedSheetData(sheet_data) => { result.sheet.update(sheet_data); },
                PostingChange::AddPart(part)                => { result.parts.insert(part); },
                PostingChange::ChangePartQty(part)          => { result.parts.insert(part); },
                PostingChange::DeletePart(part)             => { result.parts.remove(&part); },
            }
        }

        Ok(result)
    }
}
