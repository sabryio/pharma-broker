//! Participant repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::*;
use uuid::Uuid;

use crate::entity::common::MatchStatus;
use crate::entity::group;
use crate::entity::match_::Entity as Match;
use crate::entity::offer::Entity as Offer;
use crate::entity::participant::{self, Entity as Participant};
use crate::entity::participant_group::{self, Entity as ParticipantGroup};
use crate::entity::request::Entity as Request;
use crate::traits::{ParticipantRepository, ParticipantStats};
use crate::{Error, Result};

/// SeaORM-based participant repository
pub struct SeaOrmParticipantRepo {
    db: Arc<DatabaseConnection>,
}

impl SeaOrmParticipantRepo {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ParticipantRepository for SeaOrmParticipantRepo {
    async fn get_by_id(&self, id: Uuid) -> Result<Option<participant::Model>> {
        Participant::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_jid(&self, jid: &str) -> Result<Option<participant::Model>> {
        Participant::find()
            .filter(participant::Column::Jid.eq(jid))
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_by_phone(&self, phone: &str) -> Result<Option<participant::Model>> {
        Participant::find()
            .filter(participant::Column::Phone.eq(phone))
            .one(&*self.db)
            .await
            .map_err(Error::from)
    }

    async fn save(&self, model: &participant::Model) -> Result<participant::Model> {
        let existing = self.get_by_id(model.id).await?;
        let active: participant::ActiveModel = model.clone().into();

        if existing.is_some() {
            active.update(&*self.db).await.map_err(Error::from)
        } else {
            active.insert(&*self.db).await.map_err(Error::from)
        }
    }

    async fn get_groups(&self, participant_id: Uuid) -> Result<Vec<group::Model>> {
        Participant::find_by_id(participant_id)
            .find_with_related(group::Entity)
            .all(&*self.db)
            .await
            .map(|res| res.into_iter().flat_map(|(_, groups)| groups).collect())
            .map_err(Error::from)
    }

    async fn add_to_group(&self, participant_id: Uuid, group_id: Uuid) -> Result<()> {
        let active = participant_group::ActiveModel {
            participant_id: Set(participant_id),
            group_id: Set(group_id),
            last_seen_at: Set(chrono::Utc::now()),
        };

        ParticipantGroup::insert(active)
            .on_conflict(
                sea_query::OnConflict::columns([
                    participant_group::Column::ParticipantId,
                    participant_group::Column::GroupId,
                ])
                .update_column(participant_group::Column::LastSeenAt)
                .to_owned(),
            )
            .exec(&*self.db)
            .await?;

        Ok(())
    }

    async fn get_stats(&self, participant_id: Uuid) -> Result<ParticipantStats> {
        use crate::entity::{match_, offer, request};

        // Count offers by this participant
        let total_offers = Offer::find()
            .filter(offer::Column::ParticipantId.eq(participant_id))
            .count(&*self.db)
            .await
            .map_err(Error::from)? as i64;

        // Count requests by this participant
        let total_requests = Request::find()
            .filter(request::Column::ParticipantId.eq(participant_id))
            .count(&*self.db)
            .await
            .map_err(Error::from)? as i64;

        // Get offer IDs for this participant
        let offer_ids: Vec<Uuid> = Offer::find()
            .filter(offer::Column::ParticipantId.eq(participant_id))
            .select_only()
            .column(offer::Column::Id)
            .into_tuple()
            .all(&*self.db)
            .await
            .map_err(Error::from)?;

        // Get request IDs for this participant
        let request_ids: Vec<Uuid> = Request::find()
            .filter(request::Column::ParticipantId.eq(participant_id))
            .select_only()
            .column(request::Column::Id)
            .into_tuple()
            .all(&*self.db)
            .await
            .map_err(Error::from)?;

        // Count confirmed matches involving this participant's offers or requests
        let confirmed_matches = if offer_ids.is_empty() && request_ids.is_empty() {
            0i64
        } else {
            let mut query = Match::find().filter(match_::Column::Status.eq(MatchStatus::Confirmed));

            if !offer_ids.is_empty() && !request_ids.is_empty() {
                query = query.filter(
                    Condition::any()
                        .add(match_::Column::OfferId.is_in(offer_ids.clone()))
                        .add(match_::Column::RequestId.is_in(request_ids.clone())),
                );
            } else if !offer_ids.is_empty() {
                query = query.filter(match_::Column::OfferId.is_in(offer_ids.clone()));
            } else {
                query = query.filter(match_::Column::RequestId.is_in(request_ids.clone()));
            }

            query.count(&*self.db).await.map_err(Error::from)? as i64
        };

        // Count rejected matches
        let rejected_matches = if offer_ids.is_empty() && request_ids.is_empty() {
            0i64
        } else {
            let mut query = Match::find().filter(match_::Column::Status.eq(MatchStatus::Rejected));

            if !offer_ids.is_empty() && !request_ids.is_empty() {
                query = query.filter(
                    Condition::any()
                        .add(match_::Column::OfferId.is_in(offer_ids.clone()))
                        .add(match_::Column::RequestId.is_in(request_ids.clone())),
                );
            } else if !offer_ids.is_empty() {
                query = query.filter(match_::Column::OfferId.is_in(offer_ids.clone()));
            } else {
                query = query.filter(match_::Column::RequestId.is_in(request_ids.clone()));
            }

            query.count(&*self.db).await.map_err(Error::from)? as i64
        };

        // Calculate approval rate
        let total_reviewed = confirmed_matches + rejected_matches;
        let approval_rate = if total_reviewed > 0 {
            (confirmed_matches as f64 / total_reviewed as f64) * 100.0
        } else {
            0.0
        };

        // Get average confidence of confirmed matches
        let avg_confidence = if offer_ids.is_empty() && request_ids.is_empty() {
            0.0
        } else {
            let mut query = Match::find().filter(match_::Column::Status.eq(MatchStatus::Confirmed));

            if !offer_ids.is_empty() && !request_ids.is_empty() {
                query = query.filter(
                    Condition::any()
                        .add(match_::Column::OfferId.is_in(offer_ids.clone()))
                        .add(match_::Column::RequestId.is_in(request_ids.clone())),
                );
            } else if !offer_ids.is_empty() {
                query = query.filter(match_::Column::OfferId.is_in(offer_ids.clone()));
            } else {
                query = query.filter(match_::Column::RequestId.is_in(request_ids.clone()));
            }

            let matches = query.all(&*self.db).await.map_err(Error::from)?;
            if matches.is_empty() {
                0.0
            } else {
                let sum: f64 = matches.iter().map(|m| m.score).sum();
                (sum / matches.len() as f64) * 100.0
            }
        };

        // Get last activity (most recent offer or request)
        let last_offer = Offer::find()
            .filter(offer::Column::ParticipantId.eq(participant_id))
            .order_by_desc(offer::Column::CreatedAt)
            .one(&*self.db)
            .await
            .map_err(Error::from)?;

        let last_request = Request::find()
            .filter(request::Column::ParticipantId.eq(participant_id))
            .order_by_desc(request::Column::CreatedAt)
            .one(&*self.db)
            .await
            .map_err(Error::from)?;

        let last_activity = match (last_offer, last_request) {
            (Some(o), Some(r)) => Some(o.created_at.max(r.created_at)),
            (Some(o), None) => Some(o.created_at),
            (None, Some(r)) => Some(r.created_at),
            (None, None) => None,
        };

        // Determine reputation based on activity and approval rate
        let total_items = total_offers + total_requests;
        let reputation = if total_items < 5 {
            "new".to_string()
        } else if total_items >= 20 && approval_rate >= 80.0 {
            "trusted".to_string()
        } else {
            "regular".to_string()
        };

        Ok(ParticipantStats {
            participant_id,
            total_offers,
            total_requests,
            confirmed_matches,
            rejected_matches,
            approval_rate,
            avg_confidence,
            last_activity,
            reputation,
        })
    }
}
