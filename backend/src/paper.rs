use crate::repository::paper_account::PaperAccountRepository;
use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug, Default, Serialize)]
pub struct PortfolioRefreshReport {
    pub accounts_recorded: usize,
    pub accounts_skipped: usize,
}

pub struct PortfolioRefreshService {
    repository: PaperAccountRepository,
}

impl PortfolioRefreshService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repository: PaperAccountRepository::new(pool),
        }
    }

    pub async fn refresh(&self) -> Result<PortfolioRefreshReport, sqlx::Error> {
        let mut report = PortfolioRefreshReport::default();
        for (user_id, account_id) in self.repository.active_account_owners().await? {
            if self
                .repository
                .record_snapshot(user_id, account_id)
                .await?
                .is_some()
            {
                report.accounts_recorded += 1;
            } else {
                report.accounts_skipped += 1;
            }
        }
        Ok(report)
    }
}
