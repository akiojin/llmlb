//! Read boundaries are applied inside SQL, before aggregation and pagination.

use super::RequestHistoryStorage;
use crate::auth::scope::UsageScope;

/// Only canonical UUID values from server authorization enter these predicates.
/// All caller-controlled filters continue to use bound SQL parameters.
pub(crate) fn scope_predicate(scope: &UsageScope) -> String {
    match scope {
        UsageScope::Global => "1 = 1".into(),
        UsageScope::Organization(id) => format!("organization_id = '{id}'"),
        UsageScope::Account {
            organization_id,
            account_id,
        } => {
            format!("organization_id = '{organization_id}' AND account_id = '{account_id}'")
        }
    }
}

impl RequestHistoryStorage {
    /// Return a read-scoped handle without mutating the shared storage handle.
    pub fn scoped(&self, scope: UsageScope) -> Self {
        Self {
            pool: self.pool.clone(),
            read_scope: Some(scope),
            usage_writer: self.usage_writer.clone(),
        }
    }

    /// Restrict the source relation so every count, grouping and IP expansion
    /// sees the same authorized rows. Statements here are repository constants.
    pub(super) fn scoped_sql(&self, sql: &str) -> String {
        match self.read_scope.as_ref() {
            None | Some(UsageScope::Global) => sql.into(),
            Some(scope) => sql.replace(
                "FROM request_history",
                &format!(
                    "FROM (SELECT * FROM request_history WHERE {})",
                    scope_predicate(scope)
                ),
            ),
        }
    }
}
