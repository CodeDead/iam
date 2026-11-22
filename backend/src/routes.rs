use actix_web::{HttpResponse, guard, web};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    config::settings::RateLimitSettings,
    middleware::{
        JwtAuthMiddleware, RateLimitMiddleware, require_all_permissions, require_permission,
    },
    openapi::ApiDoc,
    web::controller::{
        audit as audit_ctrl, auth as auth_ctrl, dashboard as dashboard_ctrl, health,
        permissions as perm_ctrl, profile as profile_ctrl, roles as role_ctrl, users as user_ctrl,
    },
};

pub fn configure_routes(cfg: &mut web::ServiceConfig, rate_limit: &RateLimitSettings) {
    let auth_limiter =
        RateLimitMiddleware::per_minute(rate_limit.enabled, rate_limit.auth_requests_per_minute);
    let openapi = ApiDoc::openapi();

    cfg.service(web::resource("/health").route(web::get().to(health::liveness)))
        .service(web::resource("/health/ready").route(web::get().to(health::readiness)))
        .service(SwaggerUi::new("/swagger-ui/{_:.*}").url("/api/v1/openapi.json", openapi.clone()))
        .service(
            web::scope("/api/v1")
                .service(web::resource("/openapi.json").route(web::get().to(move || {
                    let doc = openapi.clone();
                    async move { HttpResponse::Ok().json(doc) }
                })))
                // Public authentication endpoints (rate-limited)
                .service(
                    web::scope("/auth")
                        .wrap(auth_limiter)
                        .service(
                            web::resource("/register").route(web::post().to(auth_ctrl::register)),
                        )
                        .service(web::resource("/login").route(web::post().to(auth_ctrl::login)))
                        .service(
                            web::resource("/refresh").route(web::post().to(auth_ctrl::refresh)),
                        )
                        .service(web::resource("/logout").route(web::post().to(auth_ctrl::logout)))
                        .service(
                            web::resource("/me")
                                .wrap(JwtAuthMiddleware)
                                .route(web::get().to(auth_ctrl::me)),
                        ),
                )
                // Protected dashboard endpoints
                .service(
                    web::scope("/dashboard").wrap(JwtAuthMiddleware).service(
                        web::resource("/stats")
                            .route(web::get().to(dashboard_ctrl::get_dashboard_stats)),
                    ),
                )
                // Protected self-service profile endpoints
                .service(
                    web::scope("/profile")
                        .wrap(JwtAuthMiddleware)
                        .service(
                            web::resource("")
                                .guard(guard::Get())
                                .wrap(require_permission("profile.read"))
                                .to(profile_ctrl::get_profile),
                        )
                        .service(
                            web::resource("")
                                .guard(guard::Put())
                                .wrap(require_permission("profile.update"))
                                .to(profile_ctrl::update_profile),
                        )
                        .service(
                            web::resource("")
                                .guard(guard::Delete())
                                .wrap(require_permission("profile.delete"))
                                .to(profile_ctrl::delete_account),
                        )
                        .service(
                            web::resource("/password")
                                .guard(guard::Patch())
                                .wrap(require_permission("profile.update"))
                                .to(profile_ctrl::change_password),
                        ),
                )
                // Protected user administration endpoints
                .service(
                    web::scope("/users")
                        .wrap(JwtAuthMiddleware)
                        .service(
                            web::resource("")
                                .guard(guard::Get())
                                .wrap(require_permission("users.read"))
                                .to(user_ctrl::list_users),
                        )
                        .service(
                            web::resource("")
                                .guard(guard::Post())
                                .wrap(require_permission("users.create"))
                                .to(user_ctrl::create_user),
                        )
                        .service(
                            web::resource("/{id}")
                                .guard(guard::Get())
                                .wrap(require_permission("users.read"))
                                .to(user_ctrl::get_user),
                        )
                        .service(
                            web::resource("/{id}")
                                .guard(guard::Put())
                                .wrap(require_permission("users.update"))
                                .to(user_ctrl::update_user),
                        )
                        .service(
                            web::resource("/{id}")
                                .guard(guard::Delete())
                                .wrap(require_permission("users.delete"))
                                .to(user_ctrl::delete_user),
                        )
                        .service(
                            web::resource("/{id}/status")
                                .guard(guard::Patch())
                                .wrap(require_permission("users.update"))
                                .to(user_ctrl::update_user_status),
                        )
                        .service(
                            web::resource("/{id}/roles")
                                .guard(guard::Get())
                                .wrap(require_permission("users.read"))
                                .to(user_ctrl::get_user_roles),
                        )
                        .service(
                            web::resource("/{id}/roles")
                                .guard(guard::Put())
                                .wrap(require_permission("users.assign_roles"))
                                .to(user_ctrl::update_user_roles),
                        )
                        .service(
                            web::resource("/{id}/permissions")
                                .guard(guard::Get())
                                .wrap(require_permission("users.read"))
                                .to(user_ctrl::get_user_permissions),
                        ),
                )
                // Protected role administration endpoints
                .service(
                    web::scope("/roles")
                        .wrap(JwtAuthMiddleware)
                        .service(
                            web::resource("")
                                .guard(guard::Get())
                                .wrap(require_permission("roles.read"))
                                .to(role_ctrl::list_roles),
                        )
                        .service(
                            web::resource("")
                                .guard(guard::Post())
                                .wrap(require_permission("roles.create"))
                                .to(role_ctrl::create_role),
                        )
                        .service(
                            web::resource("/{id}")
                                .guard(guard::Get())
                                .wrap(require_permission("roles.read"))
                                .to(role_ctrl::get_role),
                        )
                        .service(
                            web::resource("/{id}")
                                .guard(guard::Put())
                                .wrap(require_permission("roles.update"))
                                .to(role_ctrl::update_role),
                        )
                        .service(
                            web::resource("/{id}")
                                .guard(guard::Delete())
                                .wrap(require_permission("roles.delete"))
                                .to(role_ctrl::delete_role),
                        )
                        .service(
                            web::resource("/{id}/permissions")
                                .guard(guard::Get())
                                .wrap(require_permission("roles.read"))
                                .to(role_ctrl::get_role_permissions),
                        )
                        .service(
                            web::resource("/{id}/permissions")
                                .guard(guard::Put())
                                .wrap(require_permission("roles.update"))
                                .to(role_ctrl::update_role_permissions),
                        )
                        .service(
                            web::resource("/{id}/users")
                                .guard(guard::Get())
                                .wrap(require_all_permissions(&["roles.read", "users.read"]))
                                .to(role_ctrl::get_role_users),
                        ),
                )
                // Protected permission administration endpoints
                .service(
                    web::scope("/permissions")
                        .wrap(JwtAuthMiddleware)
                        .service(
                            web::resource("")
                                .guard(guard::Get())
                                .wrap(require_permission("permissions.read"))
                                .to(perm_ctrl::list_permissions),
                        )
                        .service(
                            web::resource("")
                                .guard(guard::Post())
                                .wrap(require_permission("permissions.create"))
                                .to(perm_ctrl::create_permission),
                        )
                        .service(
                            web::resource("/{id}")
                                .guard(guard::Get())
                                .wrap(require_permission("permissions.read"))
                                .to(perm_ctrl::get_permission),
                        )
                        .service(
                            web::resource("/{id}")
                                .guard(guard::Put())
                                .wrap(require_permission("permissions.update"))
                                .to(perm_ctrl::update_permission),
                        )
                        .service(
                            web::resource("/{id}")
                                .guard(guard::Delete())
                                .wrap(require_permission("permissions.delete"))
                                .to(perm_ctrl::delete_permission),
                        )
                        .service(
                            web::resource("/{id}/roles")
                                .guard(guard::Get())
                                .wrap(require_all_permissions(&["permissions.read", "roles.read"]))
                                .to(perm_ctrl::get_permission_roles),
                        ),
                )
                // Protected audit log endpoints
                .service(
                    web::scope("/audit-logs")
                        .wrap(JwtAuthMiddleware)
                        .service(
                            web::resource("")
                                .guard(guard::Get())
                                .wrap(require_permission("audit_logs.read"))
                                .to(audit_ctrl::list_audit_logs),
                        )
                        .service(
                            web::resource("/{id}")
                                .guard(guard::Get())
                                .wrap(require_permission("audit_logs.read"))
                                .to(audit_ctrl::get_audit_log),
                        ),
                ),
        );
}
