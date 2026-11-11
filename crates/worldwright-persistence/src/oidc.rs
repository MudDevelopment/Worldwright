use crate::{PersistenceError, types::*};
use sha2::{Digest, Sha256};
use worldwright_core::ids::InviteId;

pub const MAX_ACTIVE_PLAYER_INVITES: usize = 1024;
pub const MIN_PLAYER_INVITE_TTL_SECONDS: u64 = 300;
pub const MAX_PLAYER_INVITE_TTL_SECONDS: u64 = 7 * 24 * 60 * 60;

impl crate::store::PostgresStore {
    pub async fn issue_oidc_invite(
        &self,
        input: OidcInviteSpec<'_>,
    ) -> Result<InviteId, PersistenceError> {
        validate_username(input.username)?;
        validate_oidc_issuer(input.issuer)?;
        if !(MIN_PLAYER_INVITE_TTL_SECONDS..=MAX_PLAYER_INVITE_TTL_SECONDS)
            .contains(&input.ttl_seconds)
        {
            return Err(PersistenceError::InvalidData);
        }
        let ttl = i64_from_u64(input.ttl_seconds, true)?;
        let mut tx = self.pool().begin().await?;
        let world: Option<(uuid::Uuid,)> = sqlx::query_as(
            "SELECT world_id FROM worldwright_worlds WHERE world_id = $1 FOR UPDATE",
        )
        .bind(uuid_of(input.world))
        .fetch_optional(&mut *tx)
        .await?;
        if world.is_none() {
            return Err(PersistenceError::NotFound);
        }
        let (active,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*)::BIGINT FROM worldwright_player_invites \
             WHERE world_id = $1 AND consumed_at IS NULL \
             AND expires_at > clock_timestamp()",
        )
        .bind(uuid_of(input.world))
        .fetch_one(&mut *tx)
        .await?;
        if u64_from_i64(active, false)? as usize >= MAX_ACTIVE_PLAYER_INVITES {
            return Err(PersistenceError::SessionCapacity);
        }
        let account: Option<(uuid::Uuid,)> = sqlx::query_as(
            "SELECT a.account_id FROM worldwright_accounts AS a \
             JOIN worldwright_characters AS c ON c.account_id = a.account_id \
             WHERE a.username = $1 AND c.world_id = $2 AND a.disabled = FALSE \
             FOR SHARE OF a, c",
        )
        .bind(input.username)
        .bind(uuid_of(input.world))
        .fetch_optional(&mut *tx)
        .await?;
        let (account,) = account.ok_or(PersistenceError::NotFound)?;
        let inserted = sqlx::query(
            "INSERT INTO worldwright_player_invites \
             (invite_id, world_id, account_id, issuer, token_hash, expires_at) \
             VALUES ($1, $2, $3, $4, $5, \
              clock_timestamp() + ($6::DOUBLE PRECISION * INTERVAL '1 second')) \
             ON CONFLICT (token_hash) DO NOTHING",
        )
        .bind(uuid_of(input.invite))
        .bind(uuid_of(input.world))
        .bind(account)
        .bind(input.issuer)
        .bind(input.token_hash.as_slice())
        .bind(ttl)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() != 1 {
            return Err(PersistenceError::SessionCollision);
        }
        // A failed COMMIT acknowledgement is not evidence of rollback. Inspect
        // uncertain invite issuance; do not claim the token was never persisted.
        tx.commit().await.map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(input.invite)
    }

    pub async fn login_oidc(
        &self,
        input: OidcLoginSpec<'_>,
    ) -> Result<OidcSession, PersistenceError> {
        validate_oidc_issuer(input.issuer)?;
        validate_oidc_subject(input.subject)?;
        if input.session_ttl_seconds == 0
            || input.max_sessions == 0
            || input.max_sessions > MAX_DURABLE_SESSIONS
        {
            return Err(PersistenceError::InvalidData);
        }
        let ttl = i64_from_u64(input.session_ttl_seconds, true)?;
        let mut tx = self.pool().begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(oidc_advisory_key(input.issuer, input.subject))
            .execute(&mut *tx)
            .await?;
        let existing: Option<(uuid::Uuid, uuid::Uuid, String)> = sqlx::query_as(
            "SELECT a.account_id, c.character_id, c.character_name \
             FROM worldwright_oidc_identities AS o \
             JOIN worldwright_accounts AS a ON a.account_id = o.account_id \
             JOIN worldwright_characters AS c ON c.account_id = a.account_id \
             WHERE o.issuer = $1 AND o.subject = $2 AND c.world_id = $3 \
              AND a.disabled = FALSE FOR UPDATE OF o, a, c",
        )
        .bind(input.issuer)
        .bind(input.subject)
        .bind(uuid_of(input.world))
        .fetch_optional(&mut *tx)
        .await?;
        let (account, character, character_name, linked, invite_id) =
            if let Some((account, character, character_name)) = existing {
                if input.invite_hash.is_some() {
                    return Err(PersistenceError::IdentityConflict);
                }
                (account, character, character_name, false, None)
            } else {
                let invite_hash = input.invite_hash.ok_or(PersistenceError::InviteInvalid)?;
                let invite: Option<(uuid::Uuid, uuid::Uuid, uuid::Uuid, String)> = sqlx::query_as(
                    "SELECT i.invite_id, a.account_id, c.character_id, c.character_name \
                         FROM worldwright_player_invites AS i \
                         JOIN worldwright_accounts AS a ON a.account_id = i.account_id \
                         JOIN worldwright_characters AS c ON c.account_id = a.account_id \
                         WHERE i.token_hash = $1 AND i.world_id = $2 AND i.issuer = $3 \
                          AND i.consumed_at IS NULL AND i.expires_at > clock_timestamp() \
                          AND c.world_id = i.world_id AND a.disabled = FALSE \
                         FOR UPDATE OF i, a, c",
                )
                .bind(invite_hash.as_slice())
                .bind(uuid_of(input.world))
                .bind(input.issuer)
                .fetch_optional(&mut *tx)
                .await?;
                let (invite, account, character, character_name) =
                    invite.ok_or(PersistenceError::InviteInvalid)?;
                let inserted = sqlx::query(
                    "INSERT INTO worldwright_oidc_identities \
                     (issuer, subject, account_id) VALUES ($1, $2, $3) \
                     ON CONFLICT (issuer, subject) DO NOTHING",
                )
                .bind(input.issuer)
                .bind(input.subject)
                .bind(account)
                .execute(&mut *tx)
                .await?;
                if inserted.rows_affected() != 1 {
                    return Err(PersistenceError::IdentityConflict);
                }
                let consumed = sqlx::query(
                    "UPDATE worldwright_player_invites SET consumed_at = clock_timestamp(), \
                     consumed_subject = $2 WHERE invite_id = $1 AND consumed_at IS NULL \
                     AND expires_at > clock_timestamp()",
                )
                .bind(invite)
                .bind(input.subject)
                .execute(&mut *tx)
                .await?;
                if consumed.rows_affected() != 1 {
                    return Err(PersistenceError::InviteInvalid);
                }
                (account, character, character_name, true, Some(invite))
            };
        validate_name(&character_name)?;
        sqlx::query("LOCK TABLE worldwright_sessions IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "DELETE FROM worldwright_sessions WHERE revoked_at IS NOT NULL \
             OR expires_at <= clock_timestamp()",
        )
        .execute(&mut *tx)
        .await?;
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*)::BIGINT FROM worldwright_sessions \
             WHERE revoked_at IS NULL AND expires_at > clock_timestamp()",
        )
        .fetch_one(&mut *tx)
        .await?;
        if u64_from_i64(count, false)? as usize >= input.max_sessions {
            return Err(PersistenceError::SessionCapacity);
        }
        let inserted = sqlx::query(
            "INSERT INTO worldwright_sessions \
             (token_hash, account_id, character_id, world_id, expires_at) \
             VALUES ($1, $2, $3, $4, \
              clock_timestamp() + ($5::DOUBLE PRECISION * INTERVAL '1 second')) \
             ON CONFLICT (token_hash) DO NOTHING",
        )
        .bind(input.session_hash.as_slice())
        .bind(account)
        .bind(character)
        .bind(uuid_of(input.world))
        .bind(ttl)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() != 1 {
            return Err(PersistenceError::SessionCollision);
        }
        sqlx::query(
            "UPDATE worldwright_oidc_identities SET last_login_at = clock_timestamp() \
             WHERE issuer = $1 AND subject = $2 AND account_id = $3",
        )
        .bind(input.issuer)
        .bind(input.subject)
        .bind(account)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO worldwright_oidc_audit \
             (world_id, account_id, issuer, subject, action, invite_id) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(uuid_of(input.world))
        .bind(account)
        .bind(input.issuer)
        .bind(input.subject)
        .bind(if linked {
            "identity_linked"
        } else {
            "identity_login"
        })
        .bind(invite_id)
        .execute(&mut *tx)
        .await?;
        // Never mint a browser cookie from an uncertain durable identity/session commit.
        tx.commit().await.map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(OidcSession {
            principal: SessionPrincipal {
                account: account_id(account)?,
                character: character_id(character)?,
                character_name,
            },
            linked,
        })
    }
}

pub fn oidc_token_hash(token: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"worldwright-oidc-token-v1\0");
    hasher.update(token.as_bytes());
    hasher.finalize().into()
}

fn validate_oidc_issuer(value: &str) -> Result<(), PersistenceError> {
    if !(8..=2048).contains(&value.len())
        || value.trim() != value
        || value.chars().any(char::is_control)
        || !value.starts_with("https://")
    {
        return Err(PersistenceError::InvalidData);
    }
    Ok(())
}

fn validate_oidc_subject(value: &str) -> Result<(), PersistenceError> {
    if value.is_empty()
        || value.len() > 255
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(PersistenceError::InvalidData);
    }
    Ok(())
}

fn oidc_advisory_key(issuer: &str, subject: &str) -> String {
    format!(
        "oidc|{}:{}|{}:{}",
        issuer.len(),
        issuer,
        subject.len(),
        subject
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oidc_values_and_advisory_scope_are_bounded() {
        assert!(validate_oidc_issuer("https://issuer.example").is_ok());
        assert!(validate_oidc_issuer("http://issuer.example").is_err());
        assert!(validate_oidc_subject("subject").is_ok());
        assert!(validate_oidc_subject(" ").is_err());
        assert_ne!(
            oidc_advisory_key("https://a.example", "b:c"),
            oidc_advisory_key("https://a.example:b", "c")
        );
        assert_ne!(oidc_token_hash("a"), oidc_token_hash("b"));
    }
}
