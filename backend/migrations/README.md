# MongoDB Schema, Indexes & Migrations

This application uses MongoDB (`mongo:latest` in Docker Compose and CI) with automated, idempotent index initialization and system RBAC seeding at startup (`src/database/mongodb.rs` and `src/database/seed.rs`).

BSON Data Access Objects (DAOs) and MongoDB repository implementations live under `src/repository/`:
- `src/repository/user/{user_repository.rs, dao/user.rs}`
- `src/repository/role/{role_repository.rs, dao/role.rs}`
- `src/repository/permission/{permission_repository.rs, dao/permission.rs}`
- `src/repository/auth/{refresh_token_repository.rs, dao/refresh_token.rs}`

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

## MongoDB Replica Set Requirement for Transactions

Multi-document ACID transactions in MongoDB require a **Replica Set** (even a single-node replica set in local development, e.g., `rs0`).

Operations using transactions:
- Refresh token rotation (revoking the consumed token and inserting its replacement atomically in `src/repository/auth/refresh_token_repository.rs`).
- Deleting a role or permission while cleaning up references in `users.roles` or `roles.permissions`.
- Updating security-sensitive user assignments together with revoking active refresh tokens.

The repository layer detects whether the connected MongoDB deployment supports multi-document transactions. In Docker Compose (`docker-compose.yml`), MongoDB is pre-configured as a single-node replica set (`rs0`) so transactions work out of the box. In standalone MongoDB instances without a replica set, the backend logs a clear startup warning and executes the operations sequentially as a fallback (production deployments MUST run a replica set).
