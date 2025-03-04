pub(crate) use internals::ConnectionHelper;

#[cfg(not(feature = "explain-query-plans"))]
mod internals {
    use rusqlite::{CachedStatement, Connection};

    pub trait ConnectionHelper {
        fn prepare_cached_opt_explain(&self, sql: &str) -> rusqlite::Result<CachedStatement<'_>>;
    }

    impl ConnectionHelper for Connection {
        fn prepare_cached_opt_explain(&self, sql: &str) -> rusqlite::Result<CachedStatement<'_>> {
            self.prepare_cached(sql)
        }
    }
}

#[cfg(feature = "explain-query-plans")]
mod internals {
    use rusqlite::{CachedStatement, Connection, Params, Row, Rows, Statement};

    pub trait ConnectionHelper {
        fn prepare_cached_opt_explain(
            &self,
            sql: &str,
        ) -> rusqlite::Result<CachedStatementWithQueryPlan<'_>>;
    }

    impl ConnectionHelper for Connection {
        fn prepare_cached_opt_explain(
            &self,
            sql: &str,
        ) -> rusqlite::Result<CachedStatementWithQueryPlan<'_>> {
            let statement = self.prepare_cached(sql)?;

            Ok(CachedStatementWithQueryPlan { conn: self, sql: sql.into(), statement })
        }
    }

    pub struct CachedStatementWithQueryPlan<'conn> {
        conn: &'conn Connection,
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
            use super::super::utils::pretty_print_query_results;

            let mut explain_stmt =
                self.conn.prepare(&format!("EXPLAIN QUERY PLAN {}", self.sql))?;

            let rows = explain_stmt.query(params)?;

            if let Some(sql) = rows.as_ref().and_then(Statement::expanded_sql) {
                println!("\n>> {sql}");
            }

            println!("{}", pretty_print_query_results(rows)?);

            Ok(())
        }
    }
}
