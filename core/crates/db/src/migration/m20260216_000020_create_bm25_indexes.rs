use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create BM25 indexes for medication matching using pg_textsearch
        // Using 'simple' text_config for multilingual support (Arabic + English)
        
        // Create BM25 index on offers.medication
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS offers_medication_bm25_idx 
                ON offers USING bm25(medication) 
                WITH (text_config='simple')
                "#,
            )
            .await?;

        // Create BM25 index on requests.medication
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS requests_medication_bm25_idx 
                ON requests USING bm25(medication) 
                WITH (text_config='simple')
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop BM25 indexes
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS offers_medication_bm25_idx")
            .await?;

        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS requests_medication_bm25_idx")
            .await?;

        Ok(())
    }
}
