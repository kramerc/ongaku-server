use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Track::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Track::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Track::Path).string().not_null().unique_key())
                    .col(ColumnDef::new(Track::Extension).string().not_null())
                    .col(ColumnDef::new(Track::Title).string().not_null())
                    .col(ColumnDef::new(Track::Artist).string().not_null())
                    .col(ColumnDef::new(Track::Album).string().not_null())
                    .col(ColumnDef::new(Track::Genre).string().not_null())
                    .col(ColumnDef::new(Track::AlbumArtist).string().not_null())
                    .col(ColumnDef::new(Track::Publisher).string().not_null())
                    .col(ColumnDef::new(Track::CatalogNumber).string().not_null())
                    .col(ColumnDef::new(Track::DurationSeconds).integer().not_null())
                    .col(ColumnDef::new(Track::AudioBitrate).integer().not_null())
                    .col(ColumnDef::new(Track::OverallBitrate).integer().not_null())
                    .col(ColumnDef::new(Track::SampleRate).integer().not_null())
                    .col(ColumnDef::new(Track::BitDepth).integer().not_null())
                    .col(ColumnDef::new(Track::Channels).integer().not_null())
                    .col(ColumnDef::new(Track::Tags).text().not_null())
                    .col(ColumnDef::new(Track::Created).date_time().not_null())
                    .col(ColumnDef::new(Track::Modified).date_time().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Track::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Track {
    Table,
    Id,
    Path,
    Extension,
    Title,
    Artist,
    Album,
    Genre,
    AlbumArtist,
    Publisher,
    CatalogNumber,
    DurationSeconds,
    AudioBitrate,
    OverallBitrate,
    SampleRate,
    BitDepth,
    Channels,
    Tags,
    Created,
    Modified,
}
