//! Participant repository implementation

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::*;
use uuid::Uuid;

use crate::entity::group;
use crate::entity::participant::{self, Entity as Participant};
use crate::entity::participant_group::{self, Entity as ParticipantGroup};
use crate::traits::ParticipantRepository;
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
}
