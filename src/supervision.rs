//! Operator-issued run supervision; renewal credentials never admit tools.
use super::*;

impl Deadbolt {
    /// Renew a still-live supervised root and its still-live registered children.
    /// The separate root-bound credential cannot admit, change policy, unpause,
    /// revive expired/killed leases, or extend the immutable hard deadline.
    /// Authorization, expiry checks, mutation and primary evidence share one
    /// writer transaction. Keep this credential solely in trusted supervision.
    pub fn renew_supervised(&self, agent_id: &str, secret: &str) -> Result<(), DeadboltError> {
        require_token(agent_id)?;
        if !self.enabled {
            return Err(DeadboltError::Disabled);
        }
        let hash = credentials::credential_hash_for(secret, "dbs1-")?;
        self.operator_event("supervisor_renew", |tx| {
            // Read time after the writer lock, never before an authorization wait.
            let now = now_secs();
            let valid: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM admission_credentials WHERE agent_id=?1 AND token_hash=?2 AND scope='supervisor' AND expires_at>?3 AND revoked_at IS NULL)",
                params![agent_id, hash, now],
                |row| row.get(0),
            ).map_err(|_| DeadboltError::StoreUnavailable)?;
            if !valid {
                return Err(DeadboltError::TokenRequired);
            }
            let root = load_lease(tx, agent_id)
                .map_err(|_| DeadboltError::StoreUnavailable)?
                .ok_or(DeadboltError::NotFound)?;
            if root.supervision_root.as_deref() != Some(agent_id) {
                return Err(DeadboltError::BadRequest);
            }
            if root.state == "killed" {
                return Err(DeadboltError::Killed);
            }
            if deadline_reached(&root, now) || now >= root.expires_at {
                return Err(DeadboltError::BadRequest);
            }
            // Never shorten a previously issued liveness window on config change.
            let expiry = bounded_expiry(now, self.ttl, root.hard_deadline_at).max(root.expires_at);
            let descendants = descendant_ids(tx, agent_id)
                .map_err(|_| DeadboltError::StoreUnavailable)?;
            let mut renewed = Vec::new();
            for id in std::iter::once(agent_id.to_string()).chain(descendants) {
                let lease = load_lease(tx, &id)
                    .map_err(|_| DeadboltError::StoreUnavailable)?
                    .ok_or(DeadboltError::StoreUnavailable)?;
                if lease.supervision_root.as_deref() != Some(agent_id)
                    || lease.state == "killed"
                    || now >= lease.expires_at
                    || deadline_reached(&lease, now)
                {
                    continue;
                }
                let target = lease.hard_deadline_at.map_or(expiry, |deadline| expiry.min(deadline));
                tx.execute(
                    "UPDATE leases SET expires_at=?1, updated_at=?2 WHERE agent_id=?3",
                    params![target, now, id],
                ).map_err(|_| DeadboltError::StoreUnavailable)?;
                renewed.push(id);
            }
            Ok(json!({"agent_id": agent_id, "expires_at": expiry, "renewed": renewed}))
        })
    }
}
