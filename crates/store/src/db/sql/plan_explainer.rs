use rusqlite::{CachedStatement, Params, Row, Rows, Statement};
use termtree::Tree;

use crate::db::{connection::Connection, transaction::Transaction};

impl Connection {
    #[inline]
    pub fn prepare_cached(&self, sql: &str) -> rusqlite::Result<CachedStatementWithQueryPlan<'_>> {
        let statement = self.inner().prepare_cached(sql)?;

        Ok(CachedStatementWithQueryPlan {
            executor: Executor::Connection(self.inner()),
            sql: sql.into(),
            statement,
        })
    }

    #[inline]
    pub fn execute<P: Params + Clone>(&self, sql: &str, params: P) -> rusqlite::Result<usize> {
        self.prepare_cached(sql)?.execute(params)
    }

    #[inline]
    pub fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> rusqlite::Result<T>
    where
        P: Params + Clone,
        F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    {
        self.prepare_cached(sql)?.query_row(params, f)
    }
}

impl Transaction<'_> {
    pub fn prepare_cached(&self, sql: &str) -> rusqlite::Result<CachedStatementWithQueryPlan> {
        let statement = self.inner().prepare_cached(sql)?;

        Ok(CachedStatementWithQueryPlan {
            executor: Executor::Transaction(self.inner()),
            sql: sql.into(),
            statement,
        })
    }
}

enum Executor<'conn> {
    Connection(&'conn rusqlite::Connection),
    Transaction(&'conn rusqlite::Transaction<'conn>),
}

pub struct CachedStatementWithQueryPlan<'conn> {
    executor: Executor<'conn>,
    sql: Box<str>,
    statement: CachedStatement<'conn>,
}

impl CachedStatementWithQueryPlan<'_> {
    pub fn query<P: Params + Clone>(&mut self, params: P) -> rusqlite::Result<Rows<'_>> {
        self.explain(params.clone())?;
        self.statement.query(params)
    }

    pub fn query_row<T, P, F>(&mut self, params: P, f: F) -> rusqlite::Result<T>
    where
        P: Params + Clone,
        F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    {
        self.explain(params.clone())?;
        self.statement.query_row(params, f)
    }

    pub fn execute<P: Params + Clone>(&mut self, params: P) -> rusqlite::Result<usize> {
        self.explain(params.clone())?;
        self.statement.execute(params)
    }

    fn explain<P: Params>(&mut self, params: P) -> rusqlite::Result<()> {
        /// Path needed for storing opened nodes from the root.
        struct OpenedPath {
            path: Vec<(u64, Tree<String>)>,
        }

        impl OpenedPath {
            fn push(&mut self, id: u64, element: Tree<String>) {
                self.path.push((id, element));
            }

            fn pop(&mut self) -> Tree<String> {
                self.path.pop().expect("Stack must contain at least root node").1
            }

            fn fold_up_to(&mut self, id: u64) {
                while self.path.last().expect("Stack must contain at least root node").0 > id {
                    let top = self.pop();
                    self.path
                        .last_mut()
                        .map(|(_, last)| last.push(top))
                        .expect("Stack must contain at least root node");
                }
            }
        }

        let explain_sql = format!("EXPLAIN QUERY PLAN {}", self.sql);
        let mut explain_stmt = match self.executor {
            Executor::Connection(conn) => conn.prepare(&explain_sql)?,
            Executor::Transaction(transaction) => transaction.prepare(&explain_sql)?,
        };

        let mut rows = explain_stmt.query(params)?;

        if let Some(sql) = rows.as_ref().and_then(Statement::expanded_sql) {
            println!("\n>> {sql}");
        }

        // Build tree from the returned table
        let mut path = OpenedPath {
            path: vec![(0_u64, Tree::new("QUERY PLAN".to_string()))],
        };
        while let Some(row) = rows.next()? {
            let id: u64 = row.get(0)?;
            let parent_id: u64 = row.get(1)?;
            let not_used: bool = row.get(2)?;
            let mut label: String = row.get(3)?;
            if not_used {
                label = format!("(not used) {label}");
            }

            path.fold_up_to(parent_id);
            path.push(id, label.into());
        }
        path.fold_up_to(0);

        println!("{}", path.pop());

        Ok(())
    }
}
