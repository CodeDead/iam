use std::{collections::HashMap, sync::Arc};

use chrono::{Duration, Utc};
use mongodb::bson::DateTime as BsonDateTime;

use crate::{
    common::{errors::AppError, pagination::SortOrder},
    database::mongodb::bson_dt_to_chrono,
    middleware::AuthenticatedUser,
    repository::{
        audit::audit_log_repository::{AuditLogFilter, AuditLogRepository},
        auth::refresh_token_repository::RefreshTokenRepository,
        permission::permission_repository::PermissionRepository,
        role::role_repository::{RoleFilter, RoleRepository},
        user::{dao::user::AccountStatus, user_repository::UserRepository},
    },
    web::dto::dashboard::{
        AuditStats, DailyActivityPoint, DashboardStatsResponse, PermissionStats, RecentAuditEvent,
        ResourceCountItem, RoleDistributionItem, RoleStats, UserStats,
    },
};

const ACTIVITY_WINDOW_DAYS: i64 = 7;
const MAX_DASHBOARD_ROLES: u64 = 20;
const MAX_RECENT_AUDIT_EVENTS: u64 = 6;

#[derive(Clone)]
pub struct DashboardService {
    user_repo: Arc<dyn UserRepository>,
    role_repo: Arc<dyn RoleRepository>,
    permission_repo: Arc<dyn PermissionRepository>,
    refresh_token_repo: Arc<dyn RefreshTokenRepository>,
    audit_repo: Arc<dyn AuditLogRepository>,
}

impl DashboardService {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        role_repo: Arc<dyn RoleRepository>,
        permission_repo: Arc<dyn PermissionRepository>,
        refresh_token_repo: Arc<dyn RefreshTokenRepository>,
        audit_repo: Arc<dyn AuditLogRepository>,
    ) -> Self {
        Self {
            user_repo,
            role_repo,
            permission_repo,
            refresh_token_repo,
            audit_repo,
        }
    }

    pub async fn get_stats(
        &self,
        actor: &AuthenticatedUser,
    ) -> Result<DashboardStatsResponse, AppError> {
        let now = Utc::now();
        let since_24h = BsonDateTime::from_millis((now - Duration::hours(24)).timestamp_millis());
        let since_7d = BsonDateTime::from_millis(
            (now - Duration::days(ACTIVITY_WINDOW_DAYS)).timestamp_millis(),
        );

        let can_read_users = actor.has_permission("users.read");
        let can_read_roles = actor.has_permission("roles.read");
        let can_read_permissions = actor.has_permission("permissions.read");
        let can_read_audit = actor.has_permission("audit_logs.read");

        let my_active_sessions = self
            .refresh_token_repo
            .count_active_for_user(actor.user_id)
            .await?;

        let users = if can_read_users {
            let total = self.user_repo.count_all().await?;
            let active = self
                .user_repo
                .count_by_status(AccountStatus::Active)
                .await?;
            let disabled = self
                .user_repo
                .count_by_status(AccountStatus::Disabled)
                .await?;
            let locked = self
                .user_repo
                .count_by_status(AccountStatus::Locked)
                .await?;
            let pending_verification = self
                .user_repo
                .count_by_status(AccountStatus::PendingVerification)
                .await?;
            let new_last_7_days = self.user_repo.count_created_since(since_7d).await?;
            let active_sessions = self.refresh_token_repo.count_active_all().await?;
            Some(UserStats {
                total,
                active,
                disabled,
                locked,
                pending_verification,
                new_last_7_days,
                active_sessions,
            })
        } else {
            None
        };

        let (total_roles, roles) = if can_read_roles {
            let total = self.role_repo.count_all().await?;
            let system = self.role_repo.count_by_system(true).await?;
            let custom = total.saturating_sub(system);
            let (role_docs, _) = self
                .role_repo
                .find_all(&RoleFilter {
                    search: None,
                    skip: 0,
                    limit: MAX_DASHBOARD_ROLES,
                    sort_by: "name".to_string(),
                    sort_order: Some(SortOrder::Asc),
                })
                .await?;

            let mut distribution = Vec::with_capacity(role_docs.len());
            for role in role_docs {
                let user_count = if can_read_users {
                    Some(self.user_repo.count_by_role_id(role.id).await?)
                } else {
                    None
                };
                distribution.push(RoleDistributionItem {
                    id: role.id.to_hex(),
                    name: role.name,
                    is_system: role.is_system,
                    permission_count: role.permissions.len() as u64,
                    user_count,
                });
            }

            distribution.sort_by(|a, b| {
                b.user_count
                    .unwrap_or(0)
                    .cmp(&a.user_count.unwrap_or(0))
                    .then_with(|| b.permission_count.cmp(&a.permission_count))
                    .then_with(|| a.name.cmp(&b.name))
            });

            (
                Some(total),
                Some(RoleStats {
                    total,
                    system,
                    custom,
                    distribution,
                }),
            )
        } else {
            (None, None)
        };

        let (total_permissions, permissions) = if can_read_permissions {
            let total = self.permission_repo.count_all().await?;
            let system = self.permission_repo.count_by_system(true).await?;
            let custom = total.saturating_sub(system);
            let by_resource = self
                .permission_repo
                .count_by_resource()
                .await?
                .into_iter()
                .map(|(resource, count)| ResourceCountItem { resource, count })
                .collect();

            (
                Some(total),
                Some(PermissionStats {
                    total,
                    system,
                    custom,
                    by_resource,
                }),
            )
        } else {
            (None, None)
        };

        let audit = if can_read_audit {
            let total = self.audit_repo.count_all().await?;
            let events_last_24h = self.audit_repo.count_since(since_24h).await?;
            let events_last_7d = self.audit_repo.count_since(since_7d).await?;
            let failed_logins_last_24h = self
                .audit_repo
                .count_action_since("auth.login_failed", since_24h)
                .await?;
            let token_reuse_alerts_last_24h = self
                .audit_repo
                .count_action_since("auth.refresh_token_reused", since_24h)
                .await?;
            let token_reuse_alerts_last_7d = self
                .audit_repo
                .count_action_since("auth.refresh_token_reused", since_7d)
                .await?;
            let latest_token_reuse_at = if token_reuse_alerts_last_24h > 0 {
                let (latest, _) = self
                    .audit_repo
                    .find_all(&AuditLogFilter {
                        action: Some("auth.refresh_token_reused".to_string()),
                        skip: 0,
                        limit: 1,
                        sort_order: Some(SortOrder::Desc),
                        ..Default::default()
                    })
                    .await?;
                latest
                    .first()
                    .map(|entry| bson_dt_to_chrono(entry.created_at))
            } else {
                None
            };

            // Build a continuous 7-day UTC series ending today so charts always render 7 days.
            let today_utc = now.date_naive();
            let series_start_date = today_utc - Duration::days(ACTIVITY_WINDOW_DAYS - 1);
            let series_since = series_start_date
                .and_hms_opt(0, 0, 0)
                .map(|dt| BsonDateTime::from_millis(dt.and_utc().timestamp_millis()))
                .unwrap_or(since_7d);

            let raw_buckets = self
                .audit_repo
                .aggregate_daily_activity(series_since)
                .await?;
            let bucket_map: HashMap<String, _> = raw_buckets
                .into_iter()
                .map(|b| (b.date.clone(), b))
                .collect();

            let mut daily_activity = Vec::with_capacity(ACTIVITY_WINDOW_DAYS as usize);
            for offset in 0..ACTIVITY_WINDOW_DAYS {
                let day = series_start_date + Duration::days(offset);
                let key = day.format("%Y-%m-%d").to_string();
                if let Some(bucket) = bucket_map.get(&key) {
                    daily_activity.push(DailyActivityPoint {
                        date: key,
                        total: bucket.total,
                        auth_success: bucket.auth_success,
                        auth_failed: bucket.auth_failed,
                        admin_changes: bucket.admin_changes,
                    });
                } else {
                    daily_activity.push(DailyActivityPoint {
                        date: key,
                        total: 0,
                        auth_success: 0,
                        auth_failed: 0,
                        admin_changes: 0,
                    });
                }
            }

            let by_resource_type = self
                .audit_repo
                .count_by_resource_type_since(since_7d)
                .await?
                .into_iter()
                .map(|(resource, count)| ResourceCountItem { resource, count })
                .collect();

            let (recent_docs, _) = self
                .audit_repo
                .find_all(&AuditLogFilter {
                    skip: 0,
                    limit: MAX_RECENT_AUDIT_EVENTS,
                    sort_order: Some(SortOrder::Desc),
                    ..Default::default()
                })
                .await?;

            let recent_events = recent_docs
                .into_iter()
                .map(|entry| RecentAuditEvent {
                    id: entry.id.to_hex(),
                    action: entry.action,
                    resource_type: entry.resource_type,
                    resource_name: entry.resource_name,
                    actor_email: entry.actor_email,
                    summary: entry.summary,
                    created_at: bson_dt_to_chrono(entry.created_at),
                })
                .collect();

            Some(AuditStats {
                total,
                events_last_24h,
                events_last_7d,
                failed_logins_last_24h,
                token_reuse_alerts_last_24h,
                token_reuse_alerts_last_7d,
                latest_token_reuse_at,
                daily_activity,
                by_resource_type,
                recent_events,
            })
        } else {
            None
        };

        Ok(DashboardStatsResponse {
            my_active_sessions,
            users,
            total_roles,
            roles,
            total_permissions,
            permissions,
            audit,
        })
    }
}
