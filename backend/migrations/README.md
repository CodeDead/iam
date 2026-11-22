# MongoDB Schema, Indexes & Migrations

This application uses MongoDB with automated, idempotent index initialization and system RBAC seeding at startup ([`src/database/mongodb.rs`](../src/database/mongodb.rs) and [`src/database/seed.rs`](../src/database/seed.rs)) inside the database configured via `MONGODB_DATABASE` (defaults to `user_management`). When new system permissions (such as `audit_logs.read`) are introduced on upgrade, startup seeding automatically creates them and backfills them onto existing default system roles without overwriting custom role edits.

BSON Data Access Objects (DAOs) and repository implementations live under [`src/repository/`](../src/repository/):
- `src/repository/user/{user_repository.rs, dao/user.rs}`
- `src/repository/role/{role_repository.rs, dao/role.rs}`
- `src/repository/permission/{permission_repository.rs, dao/permission.rs}`
- `src/repository/auth/{refresh_token_repository.rs, dao/refresh_token.rs}`
- `src/repository/audit/{audit_log_repository.rs, dao/audit_log.rs}`

## Collections & Indexes

### 1. `users`
- `email`: **Unique** ascending index (`email_unique`)
- `status`: Ascending index (`status_idx`)
- `roles`: Multikey ascending index (`roles_idx`)
- `created_at`: Descending index (`created_at_desc`)

### 2. `roles`
- `name`: **Unique** ascending index (`role_name_unique`)
- `permissions`: Multikey ascending index (`permissions_idx`)
- `is_system`: Ascending index (`is_system_idx`)

### 3. `permissions`
- `name`: **Unique** ascending index (`permission_name_unique`)
- `resource`, `action`: Compound **unique** ascending index (`resource_action_unique`)
- `is_system`: Ascending index (`perm_is_system_idx`)

### 4. `refresh_tokens`
- `token_hash`: **Unique** ascending index (`token_hash_unique`)
- `user_id`: Ascending index (`user_id_idx`)
- `family_id`: Ascending index (`family_id_idx`)
- `expires_at`: TTL index (`expires_at_ttl`, `expireAfterSeconds: 0`) for automatic MongoDB cleanup of expired tokens

### 5. `audit_logs`
- `created_at`: Descending index (`audit_created_at_desc`)
- `action`, `created_at`: Compound index (`audit_action_created_at`)
- `resource_type`, `created_at`: Compound index (`audit_resource_type_created_at`)
- `actor_id`, `created_at`: Compound index (`audit_actor_id_created_at`)

## MongoDB Replica Set Requirement for Transactions

Multi-document ACID transactions in MongoDB require a **Replica Set** (even a single-node replica set in local development, e.g., `rs0`).

- **Transactional operation**: Atomic refresh-token rotation in `src/repository/auth/refresh_token_repository.rs` (revoking the consumed token and inserting its replacement within a single transaction, preventing concurrent token replay races).
- **Coordinated referential & session operations**: Deleting a custom role or permission cleans up references in `users.roles` / `roles.permissions` and immediately invalidates the in-memory `PermissionCache`; updating user roles or disabling accounts revokes active refresh tokens and invalidates cached permissions.

At startup, [`src/database/mongodb.rs`](../src/database/mongodb.rs) detects whether the connected MongoDB deployment is a replica set or sharded cluster. In `docker-compose.yml`, `mongo:latest` is pre-configured as a single-node replica set (`rs0`) so transactions work out of the box. Against standalone MongoDB instances without a replica set, the backend logs a startup warning and falls back to conditional atomic updates (production deployments MUST run a replica set).
