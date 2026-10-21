use crate::{PersistenceError, types::*};
use sqlx::{
    FromRow, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::str::FromStr;
use worldwright_core::ids::WorldId;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[derive(Clone)]
pub struct PostgresStore {
    pub(crate) pool: PgPool,
}

#[derive(FromRow)]
struct WorldRow {
    content_namespace: String,
    content_schema_version: i64,
    writer_epoch: i64,
    active_writer_id: Option<uuid::Uuid>,
}
#[derive(FromRow)]
struct IdentityRow {
    account_id: uuid::Uuid,
    username: String,
    password_hash: String,
    character_id: uuid::Uuid,
    character_name: String,
    room_definition: String,
    balance: i64,
    version: i64,
}
#[derive(FromRow)]
struct SessionRow {
    account_id: uuid::Uuid,
    character_id: uuid::Uuid,
    character_name: String,
}
#[derive(FromRow)]
struct WriterRow {
    writer_epoch: i64,
}

impl PostgresStore {
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    #[cfg(test)]
    pub(crate) fn from_pool_for_test(pool: PgPool) -> Self {
        Self::from_pool(pool)
    }

    #[cfg(test)]
    pub(crate) fn pool_for_test(&self) -> &PgPool {
        &self.pool
    }

    #[cfg(test)]
    pub(crate) async fn close_for_test(self) {
        self.pool.close().await;
    }

    pub async fn connect(database_url: &str) -> Result<Self, PersistenceError> {
        let options = PgConnectOptions::from_str(database_url)
            .map_err(|_| PersistenceError::Configuration)?;
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .after_connect(|connection, _| {
                Box::pin(async move {
                    sqlx::query("SET statement_timeout = '5s'")
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query("SET lock_timeout = '2s'")
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query("SET idle_in_transaction_session_timeout = '10s'")
                        .execute(&mut *connection)
                        .await?;
                    Ok(())
                })
            })
            .connect_with(options)
            .await?;
        Ok(Self { pool })
    }
    pub async fn migrate(&self) -> Result<(), PersistenceError> {
        MIGRATOR
            .run(&self.pool)
            .await
            .map_err(PersistenceError::Migration)
    }
    pub async fn verify_migrations(&self) -> Result<(), PersistenceError> {
        let exists: Option<(Option<String>,)> =
            sqlx::query_as("SELECT to_regclass('worldwright_worlds')::TEXT")
                .fetch_optional(&self.pool)
                .await?;
        if exists.and_then(|r| r.0).is_none() {
            return Err(PersistenceError::InvalidData);
        }
        let rows: Vec<(i64, Vec<u8>, bool)> = sqlx::query_as(
            "SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version",
        )
        .fetch_all(&self.pool)
        .await?;
        if rows.len() != MIGRATOR.migrations.len() {
            return Err(PersistenceError::InvalidData);
        }
        for (migration, row) in MIGRATOR.migrations.iter().zip(rows) {
            if migration.version != row.0
                || migration.checksum.as_ref() != row.1.as_slice()
                || !row.2
            {
                return Err(PersistenceError::InvalidData);
            }
        }
        Ok(())
    }
    pub async fn seed(&self, seed: &SeedWorld) -> Result<(), PersistenceError> {
        validate_seed(seed)?;
        let mut tx = self.pool.begin().await?;
        let world_uuid = uuid_of(seed.world);
        let world: Option<WorldRow> = sqlx::query_as("SELECT content_namespace, content_schema_version, writer_epoch, active_writer_id FROM worldwright_worlds WHERE world_id = $1 FOR UPDATE").bind(world_uuid).fetch_optional(&mut *tx).await?;
        if let Some(row) = world {
            if row.content_namespace != seed.content_namespace
                || u64_from_i64(row.content_schema_version, true)? != seed.content_schema_version
                || row.writer_epoch != 0
                || row.active_writer_id.is_some()
            {
                return Err(PersistenceError::SeedConflict);
            }
        } else {
            sqlx::query("INSERT INTO worldwright_worlds ( world_id, content_namespace, content_schema_version ) VALUES ($1, $2, $3)").bind(world_uuid).bind(&seed.content_namespace).bind(i64_from_u64(seed.content_schema_version, true)?).execute(&mut *tx).await?;
        }
        let mut accounts: Vec<&SeedAccount> = seed.accounts.iter().collect();
        accounts.sort_by_key(|a| a.account.get());
        for account in accounts {
            let rows: Vec<(uuid::Uuid, String, String, bool)> = sqlx::query_as("SELECT account_id, username, password_hash, disabled FROM worldwright_accounts WHERE account_id = $1 OR username = $2 FOR UPDATE").bind(uuid_of(account.account)).bind(&account.username).fetch_all(&mut *tx).await?;
            if rows.len() > 1 {
                return Err(PersistenceError::SeedConflict);
            }
            if let Some((id, username, password_hash, disabled)) = rows.into_iter().next() {
                if id != uuid_of(account.account)
                    || username != account.username
                    || password_hash != account.password_hash
                    || disabled
                {
                    return Err(PersistenceError::SeedConflict);
                }
            } else {
                sqlx::query("INSERT INTO worldwright_accounts ( account_id, username, password_hash ) VALUES ($1, $2, $3)").bind(uuid_of(account.account)).bind(&account.username).bind(&account.password_hash).execute(&mut *tx).await?;
            }
            let characters: Vec<(uuid::Uuid, uuid::Uuid, uuid::Uuid, String, String)> = sqlx::query_as("SELECT character_id, account_id, world_id, character_name, room_definition FROM worldwright_characters WHERE character_id = $1 OR (world_id = $3 AND account_id = $2) FOR UPDATE").bind(uuid_of(account.character)).bind(uuid_of(account.account)).bind(world_uuid).fetch_all(&mut *tx).await?;
            if characters.len() > 1 {
                return Err(PersistenceError::SeedConflict);
            }
            if let Some((id, owner, existing_world, name, room)) = characters.into_iter().next() {
                if id != uuid_of(account.character)
                    || owner != uuid_of(account.account)
                    || existing_world != world_uuid
                    || name != account.character_name
                    || room != seed.entry_room_definition
                {
                    return Err(PersistenceError::SeedConflict);
                }
            } else {
                sqlx::query("INSERT INTO worldwright_characters ( character_id, account_id, world_id, character_name, room_definition ) VALUES ($1, $2, $3, $4, $5)").bind(uuid_of(account.character)).bind(uuid_of(account.account)).bind(world_uuid).bind(&account.character_name).bind(&seed.entry_room_definition).execute(&mut *tx).await?;
            }
        }
        tx.commit().await.map_err(PersistenceError::Database)
    }
    pub async fn acquire_writer(
        &self,
        world: WorldId,
        writer: worldwright_core::ids::WriterId,
    ) -> Result<WriterLease, PersistenceError> {
        let row: Option<WriterRow> = sqlx::query_as("UPDATE worldwright_worlds SET writer_epoch = writer_epoch + 1, active_writer_id = $2, updated_at = clock_timestamp() WHERE world_id = $1 AND writer_epoch < 9223372036854775807 RETURNING writer_epoch").bind(uuid_of(world)).bind(uuid_of(writer)).fetch_optional(&self.pool).await?;
        let row = row.ok_or(PersistenceError::NotFound)?;
        Ok(WriterLease {
            world,
            writer,
            epoch: u64_from_i64(row.writer_epoch, true)?,
        })
    }
    pub async fn verify_world(
        &self,
        world: WorldId,
        namespace: &str,
        schema_version: u64,
    ) -> Result<(), PersistenceError> {
        let row: Option<(String, i64)> = sqlx::query_as("SELECT content_namespace, content_schema_version FROM worldwright_worlds WHERE world_id = $1").bind(uuid_of(world)).fetch_optional(&self.pool).await?;
        let Some((actual_namespace, actual_version)) = row else {
            return Err(PersistenceError::NotFound);
        };
        if actual_namespace != namespace || u64_from_i64(actual_version, true)? != schema_version {
            return Err(PersistenceError::InvalidData);
        }
        Ok(())
    }
    pub async fn identities(
        &self,
        world: WorldId,
    ) -> Result<Vec<IdentityRecord>, PersistenceError> {
        let rows: Vec<IdentityRow> = sqlx::query_as("SELECT a.account_id, a.username, a.password_hash, c.character_id, c.character_name, c.room_definition, c.balance, c.version FROM worldwright_accounts AS a JOIN worldwright_characters AS c ON c.account_id = a.account_id WHERE c.world_id = $1 AND a.disabled = FALSE ORDER BY c.character_id LIMIT 257").bind(uuid_of(world)).fetch_all(&self.pool).await?;
        if rows.is_empty() || rows.len() > 256 {
            return Err(PersistenceError::InvalidData);
        }
        rows.into_iter().map(identity_from_row).collect()
    }
    pub async fn identity_for_login(
        &self,
        world: WorldId,
        username: &str,
    ) -> Result<Option<IdentityRecord>, PersistenceError> {
        validate_username(username)?;
        let row: Option<IdentityRow> = sqlx::query_as("SELECT a.account_id, a.username, a.password_hash, c.character_id, c.character_name, c.room_definition, c.balance, c.version FROM worldwright_accounts AS a JOIN worldwright_characters AS c ON c.account_id = a.account_id WHERE a.username = $1 AND c.world_id = $2 AND a.disabled = FALSE").bind(username).bind(uuid_of(world)).fetch_optional(&self.pool).await?;
        row.map(identity_from_row).transpose()
    }
    pub async fn create_session(
        &self,
        principal: &SessionPrincipal,
        world: WorldId,
        token_hash: &[u8; 32],
        ttl_seconds: u64,
        max_sessions: usize,
    ) -> Result<(), PersistenceError> {
        if ttl_seconds == 0 || max_sessions == 0 || max_sessions > MAX_DURABLE_SESSIONS {
            return Err(PersistenceError::InvalidData);
        }
        let ttl = i64_from_u64(ttl_seconds, true)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("LOCK TABLE worldwright_sessions IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM worldwright_sessions WHERE revoked_at IS NOT NULL OR expires_at <= clock_timestamp()").execute(&mut *tx).await?;
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM worldwright_sessions WHERE revoked_at IS NULL AND expires_at > clock_timestamp()").fetch_one(&mut *tx).await?;
        if u64_from_i64(count, false)? as usize >= max_sessions {
            return Err(PersistenceError::SessionCapacity);
        }
        let inserted = sqlx::query("INSERT INTO worldwright_sessions ( token_hash, account_id, character_id, world_id, expires_at ) SELECT $1, a.account_id, c.character_id, c.world_id, clock_timestamp() + ($5::DOUBLE PRECISION * INTERVAL '1 second') FROM worldwright_accounts AS a JOIN worldwright_characters AS c ON c.account_id = a.account_id WHERE a.account_id = $2 AND c.character_id = $3 AND c.world_id = $4 AND a.disabled = FALSE ON CONFLICT (token_hash) DO NOTHING RETURNING expires_at").bind(token_hash.as_slice()).bind(uuid_of(principal.account)).bind(uuid_of(principal.character)).bind(uuid_of(world)).bind(ttl).fetch_optional(&mut *tx).await?;
        if inserted.is_none() {
            let (exists,): (bool,) = sqlx::query_as("SELECT EXISTS ( SELECT 1 FROM worldwright_accounts AS a JOIN worldwright_characters AS c ON c.account_id = a.account_id WHERE a.account_id = $1 AND c.character_id = $2 AND c.world_id = $3 AND a.disabled = FALSE )").bind(uuid_of(principal.account)).bind(uuid_of(principal.character)).bind(uuid_of(world)).fetch_one(&mut *tx).await?;
            if exists {
                return Err(PersistenceError::SessionCollision);
            }
            return Err(PersistenceError::Unauthorized);
        }
        tx.commit().await.map_err(PersistenceError::Database)
    }
    pub async fn session(
        &self,
        world: WorldId,
        token_hash: &[u8; 32],
    ) -> Result<Option<SessionPrincipal>, PersistenceError> {
        let row: Option<SessionRow> = sqlx::query_as("SELECT a.account_id, c.character_id, c.character_name FROM worldwright_sessions AS s JOIN worldwright_accounts AS a ON a.account_id = s.account_id JOIN worldwright_characters AS c ON c.character_id = s.character_id AND c.account_id = s.account_id AND c.world_id = s.world_id WHERE s.token_hash = $1 AND s.world_id = $2 AND s.revoked_at IS NULL AND s.expires_at > clock_timestamp() AND a.disabled = FALSE").bind(token_hash.as_slice()).bind(uuid_of(world)).fetch_optional(&self.pool).await?;
        row.map(|row| {
            let character_name = row.character_name;
            validate_name(&character_name)?;
            Ok(SessionPrincipal {
                account: account_id(row.account_id)?,
                character: character_id(row.character_id)?,
                character_name,
            })
        })
        .transpose()
    }
    pub async fn revoke_session(&self, token_hash: &[u8; 32]) -> Result<(), PersistenceError> {
        sqlx::query("UPDATE worldwright_sessions SET revoked_at = COALESCE(revoked_at, clock_timestamp()) WHERE token_hash = $1").bind(token_hash.as_slice()).execute(&self.pool).await?;
        Ok(())
    }
}

fn identity_from_row(row: IdentityRow) -> Result<IdentityRecord, PersistenceError> {
    validate_username(&row.username)?;
    validate_name(&row.character_name)?;
    validate_definition(&row.room_definition)?;
    if row.password_hash.is_empty()
        || row.password_hash.len() > 512
        || !row.password_hash.starts_with("$argon2id$")
    {
        return Err(PersistenceError::InvalidData);
    }
    Ok(IdentityRecord {
        account: account_id(row.account_id)?,
        character: character_id(row.character_id)?,
        username: row.username,
        character_name: row.character_name,
        room_definition: row.room_definition,
        balance: u64_from_i64(row.balance, false)?,
        version: u64_from_i64(row.version, true)?,
        password_hash: row.password_hash,
    })
}
fn validate_seed(seed: &SeedWorld) -> Result<(), PersistenceError> {
    validate_definition(&seed.entry_room_definition)?;
    validate_namespace(&seed.content_namespace)?;
    if seed.accounts.is_empty() || seed.accounts.len() > 256 || seed.content_schema_version == 0 {
        return Err(PersistenceError::InvalidData);
    }
    for account in &seed.accounts {
        validate_username(&account.username)?;
        validate_name(&account.character_name)?;
        if account.password_hash.is_empty()
            || account.password_hash.len() > 512
            || !account.password_hash.starts_with("$argon2id$")
        {
            return Err(PersistenceError::InvalidData);
        }
    }
    Ok(())
}
