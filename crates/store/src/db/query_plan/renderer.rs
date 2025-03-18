use rusqlite::Statement;
use termtree::Tree;

/// SQL query plan renderer which represents result as a tree using ASCII graphics.
pub struct QueryPlanRenderer {
    path: Vec<(u64, Tree<String>)>,
}

impl QueryPlanRenderer {
    /// Constructs and initializes query plan renderer.
    pub fn new() -> Self {
        Self {
            path: vec![(0_u64, Tree::new("QUERY PLAN".to_string()))],
        }
    }

    /// Runs `EXPLAIN QUERY PLAN` statement and renders result as tree using ASCII graphics.
    ///
    /// # Note
    /// Current algorithm relies on the row ordering (all child rows go after corresponding parent
    /// row) of the current implementation of SQLite's `EXPLAIN QUERY PLAN` command. This is not
    /// bad, because this makes algorithm simple and effective, and it is intended to be used only
    /// for debugging and testing.
    pub fn render_tree(mut self, mut explain_stmt: Statement) -> rusqlite::Result<String> {
        let mut rows = explain_stmt.raw_query();
        while let Some(row) = rows.next()? {
            let id: u64 = row.get(0)?;
            let parent_id: u64 = row.get(1)?;
            let label: String = row.get(3)?;

            self.fold_up_to(parent_id);
            self.path.push((id, label.into()));
        }
        self.fold_up_to(0);

        let (_, root) = self.path.pop().expect("Always present");

        Ok(root.to_string())
    }

    /// Folds all elements from the top of the path stack up to the element with the given ID.
    /// All elements with higher indexes become children of the elements with lower indexes.
    fn fold_up_to(&mut self, id: u64) {
        while self.path.last().expect("Always present").0 > id {
            let (_, top) = self.path.pop().expect("Always present");
            self.path.last_mut().map(|(_, last)| last.push(top)).expect("Always present");
        }
    }
}
