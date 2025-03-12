use crate::db::{query_plan::renderer::QueryPlanRenderer, transaction::Transaction};

pub mod renderer;

impl Transaction<'_> {
    /// Renders query plan as tree using ASCII graphics.
    pub fn explain_query_plan(&self, sql: &str) -> rusqlite::Result<()> {
        let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let explain_stmt = self.prepare(&explain_sql)?;
        let query_plan = QueryPlanRenderer::new().render_tree(explain_stmt)?;

        println!("\n>> {explain_sql}\n\n{query_plan}");

        #[cfg(test)]
        assert!(
            !query_plan_contains_unnecessary_scans(sql, &query_plan),
            "Query plan should not contain unnecessary scans:\n{query_plan}",
        );

        Ok(())
    }
}

/// Checks if the given query plan contains unnecessary table scans not utilizing indices.
#[cfg(test)]
fn query_plan_contains_unnecessary_scans(sql: &str, query_plan: &str) -> bool {
    use std::sync::LazyLock;

    static RE_IDX_SCAN: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::RegexBuilder::new(
            r"SCAN ([A-Za-z0-9_]+) (USING( COVERING)?|VIRTUAL TABLE) INDEX ([A-Za-z0-9_]+)",
        )
        .case_insensitive(true)
        .build()
        .expect("Must be a valid regex pattern")
    });

    // Preprocess query plan in order to hide some "normal" scans
    let query_plan = RE_IDX_SCAN.replace_all(query_plan, r"SEARCH $1 $2 INDEX $4");

    // Don't flag `SELECT` queries without `WHERE` clause, since they don't usually use indexes
    if sql.contains("SELECT") && !sql.contains("WHERE") {
        return false;
    }

    query_plan.contains(" SCAN ")
}
