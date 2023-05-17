
use std::collections::LinkedList;

use super::tracking::get_db;
use super::{PostingChange, ProgramStateSnapshot, TRACKING_TABLE};

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

    pub async fn get_tracked<S: AsRef<str>>(program_number: S) -> surrealdb::Result<Self> {
        get_db().await?
            .select(
                (TRACKING_TABLE, program_number.as_ref())
            ).await
    }

    pub async fn record(&self) -> surrealdb::Result<()> {
        get_db().await?
            .create((TRACKING_TABLE, &self.program))
            .content(self)
            .await?;

        Ok(())
    }

    pub async fn update_state(&mut self, state: ProgramStateSnapshot) -> surrealdb::Result<()> {
        let current_state = self.get_current_state();
        if let Some(changes) = state.calculate_changes(current_state) {
            changes
                .into_iter()
                .for_each(|change| self.changes.push_back(change));
        }

        self.flatten_repost();
        self.record().await?;

        Ok(())
    }

    pub fn get_current_state(&self) -> ProgramStateSnapshot {
        let mut iter = self.changes.iter();
        let mut initial: ProgramStateSnapshot = iter.next().unwrap().into();
        initial.name = self.program.to_string();

        // Traverse linked list to build final state
        iter.fold(initial, |acc, change| acc.update(change))
    }

    /// Flattens the dual-entry in reposting
    /// 
    /// When a program is re-posted, Sigmanest actually does a Delete and a Post (SN101, SN100).
    /// These transactions have timestamps close to eachother, but may not be the same.
    /// The order of these transactions cannot be guaranteed either.
    pub fn flatten_repost(&mut self) {
        // let mut iter = self.changes.iter_mut();
        
        let mut reposts = Vec::new();
        let mut iter = self.changes.iter().enumerate().peekable();
        while let Some((i, current)) = iter.next() {
            if let Some((_, next)) = iter.peek() {
                if PostingChange::is_reposting(current, next) {
                    // save location for removal
                    reposts.push(i);

                    // advance iterator in case next changes are a repost
                    // i.e. ... -> Post -> Delete -> Post -> Delete -> ...
                    //  will cause 3 Post/Delete pairs ot be removed
                    //  and only 1 Repost to be added
                    iter.next();
                }
            }
        }

        // iterate in reverse
        for i in reposts.into_iter().rev() {
            let mut second_half = self.changes
                .split_off(i)   // split linked list before Delete/Post transaction pair
                .split_off(2);  // drop Delete/Post pair

            // push reposting status
            self.changes.push_back(PostingChange::RePosted);
            
            // re-attach second half
            self.changes.append(&mut second_half);
        }
    }
}
