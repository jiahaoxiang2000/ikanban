use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

use crate::AppState;
use crate::entities::pr_merge;

pub async fn start(state: AppState) {
    info!("Starting PR monitoring service");
    loop {
        if let Err(e) = check_prs(&state).await {
            error!("Error checking PRs: {}", e);
        }
        sleep(Duration::from_secs(60)).await;
    }
}

async fn check_prs(state: &AppState) -> anyhow::Result<()> {
    use crate::entities::pr_merge::Entity as PrMerge;

    // Find all open PRs
    let open_prs = PrMerge::find()
        .filter(pr_merge::Column::Status.ne("merged"))
        .filter(pr_merge::Column::Status.ne("closed"))
        .all(&state.db)
        .await?;

    for _pr in open_prs {
        // TODO: Implement actual GitHub API check
        // For now, we just log that we are monitoring
        // info!("Monitoring PR: {}", pr.pr_url);

        // Mock update logic (commented out)
        /*
        let new_status = check_github_status(&pr.pr_url).await?;
        if new_status != pr.status {
            let mut active: pr_merge::ActiveModel = pr.into();
            active.status = ActiveValue::Set(new_status.clone());
            active.updated_at = ActiveValue::Set(chrono::Utc::now().naive_utc());

            if new_status == "merged" {
                active.merged_at = ActiveValue::Set(Some(chrono::Utc::now().naive_utc()));
            }

            let updated = active.update(&state.db).await?;
            state.broadcast(WsEvent::PrMergeUpdated(updated));
        }
        */
    }

    Ok(())
}
