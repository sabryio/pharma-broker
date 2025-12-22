//! MedicationMapping repository implementation

use async_trait::async_trait;
use sea_orm::*;

use crate::entity::medication_mapping::{self, Entity as MedicationMapping};
use crate::traits::MedicationMappingRepository;
use crate::{Error, Result};

/// SeaORM-based medication mapping repository
pub struct SeaOrmMedicationMappingRepo {
    db: DatabaseConnection,
}

impl SeaOrmMedicationMappingRepo {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MedicationMappingRepository for SeaOrmMedicationMappingRepo {
    async fn save(&self, model: &medication_mapping::Model) -> Result<medication_mapping::Model> {
        let existing = MedicationMapping::find_by_id(&model.id)
            .one(&self.db)
            .await?;
        let active: medication_mapping::ActiveModel = model.clone().into();

        if existing.is_some() {
            active.update(&self.db).await.map_err(Error::from)
        } else {
            active.insert(&self.db).await.map_err(Error::from)
        }
    }

    async fn find_relevant(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<medication_mapping::Model>> {
        let pattern = format!("%{}%", query);
        MedicationMapping::find()
            .filter(
                Condition::any()
                    .add(medication_mapping::Column::ArabicName.like(&pattern))
                    .add(medication_mapping::Column::EnglishName.like(&pattern)),
            )
            .order_by_asc(medication_mapping::Column::EnglishName)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn find_similar(
        &self,
        embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<medication_mapping::Model>> {
        let embedding_str = format!(
            "[{}]",
            embedding
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        MedicationMapping::find()
            .from_raw_sql(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT * FROM medication_mappings 
                WHERE embedding IS NOT NULL
                ORDER BY embedding <=> $1::vector
                LIMIT $2
                "#,
                [embedding_str.into(), limit.into()],
            ))
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn get_all(&self, limit: i64, offset: i64) -> Result<Vec<medication_mapping::Model>> {
        MedicationMapping::find()
            .order_by_asc(medication_mapping::Column::EnglishName)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(Error::from)
    }

    async fn count(&self) -> Result<i64> {
        MedicationMapping::find()
            .count(&self.db)
            .await
            .map(|c| c as i64)
            .map_err(Error::from)
    }
}

#[cfg(all(test, feature = "integration-tests"))]
mod tests {
    use super::*;
    use crate::testing::{TestDb, new_test_medication_mapping};
    use sea_orm::EntityTrait;

    async fn create_mapping(db: &TestDb, arabic: &str, english: &str) -> medication_mapping::Model {
        let mapping = new_test_medication_mapping(arabic, english);
        let id = mapping.id.clone().unwrap();
        medication_mapping::Entity::insert(mapping)
            .exec(&db.db)
            .await
            .expect("Insert mapping");

        medication_mapping::Entity::find_by_id(&id)
            .one(&db.db)
            .await
            .expect("Find mapping")
            .expect("Mapping should exist")
    }

    #[tokio::test]
    async fn test_save_insert() {
        let db = TestDb::new().await;
        let repo = SeaOrmMedicationMappingRepo::new(db.db.clone());

        let mapping = create_mapping(&db, "أسبرين", "Aspirin").await;

        // Verify via find_relevant
        let found = repo
            .find_relevant("Aspirin", 10)
            .await
            .expect("FindRelevant");
        assert!(!found.is_empty(), "Should find the mapping");
        assert!(
            found.iter().any(|m| m.id == mapping.id),
            "Should contain the mapping"
        );
    }

    #[tokio::test]
    async fn test_find_relevant_by_arabic() {
        let db = TestDb::new().await;
        let repo = SeaOrmMedicationMappingRepo::new(db.db.clone());

        create_mapping(&db, "أوجمنتين", "Augmentin").await;
        create_mapping(&db, "باراسيتامول", "Paracetamol").await;

        let results = repo
            .find_relevant("أوجمنتين", 10)
            .await
            .expect("FindRelevant");
        assert_eq!(results.len(), 1, "Should find 1 mapping");
        assert_eq!(results[0].english_name, "Augmentin");
    }

    #[tokio::test]
    async fn test_find_relevant_by_english() {
        let db = TestDb::new().await;
        let repo = SeaOrmMedicationMappingRepo::new(db.db.clone());

        create_mapping(&db, "أوجمنتين", "Augmentin").await;
        create_mapping(&db, "باراسيتامول", "Paracetamol").await;

        let results = repo.find_relevant("Para", 10).await.expect("FindRelevant");
        assert_eq!(results.len(), 1, "Should find 1 mapping");
        assert_eq!(results[0].english_name, "Paracetamol");
    }

    #[tokio::test]
    async fn test_get_all_pagination() {
        let db = TestDb::new().await;
        let repo = SeaOrmMedicationMappingRepo::new(db.db.clone());

        // Create 5 mappings
        create_mapping(&db, "أ", "Alpha").await;
        create_mapping(&db, "ب", "Beta").await;
        create_mapping(&db, "ج", "Charlie").await;
        create_mapping(&db, "د", "Delta").await;
        create_mapping(&db, "ه", "Echo").await;

        // Get first page
        let page1 = repo.get_all(2, 0).await.expect("GetAll page 1");
        assert_eq!(page1.len(), 2, "Should have 2 items on first page");

        // Get second page
        let page2 = repo.get_all(2, 2).await.expect("GetAll page 2");
        assert_eq!(page2.len(), 2, "Should have 2 items on second page");
    }

    #[tokio::test]
    async fn test_count() {
        let db = TestDb::new().await;
        let repo = SeaOrmMedicationMappingRepo::new(db.db.clone());

        assert_eq!(repo.count().await.expect("Count"), 0, "Initially 0");

        create_mapping(&db, "أ", "A").await;
        create_mapping(&db, "ب", "B").await;
        create_mapping(&db, "ج", "C").await;

        assert_eq!(
            repo.count().await.expect("Count"),
            3,
            "Should count 3 mappings"
        );
    }
}
