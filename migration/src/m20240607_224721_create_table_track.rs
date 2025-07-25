use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create the table
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
                    .col(ColumnDef::new(Track::DiscNumber).integer())
                    .col(ColumnDef::new(Track::TrackNumber).integer())
                    .col(ColumnDef::new(Track::Year).integer())
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
                    .col(ColumnDef::new(Track::Tags).json_binary().not_null())
                    .col(ColumnDef::new(Track::AlbumArtPath).string())
                    .col(ColumnDef::new(Track::AlbumArtMimeType).string())
                    .col(ColumnDef::new(Track::AlbumArtSize).integer())
                    .col(ColumnDef::new(Track::Created).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(Track::Modified).timestamp_with_time_zone().not_null())
                    .to_owned(),
            )
            .await?;

        // Create indexes for optimized scanning and querying

        // Index on modified timestamp for efficient scanning
        manager
            .create_index(
                Index::create()
                    .name("idx_track_modified")
                    .table(Track::Table)
                    .col(Track::Modified)
                    .to_owned(),
            )
            .await?;

        // Index on path for fast lookups during scanning
        manager
            .create_index(
                Index::create()
                    .name("idx_track_path")
                    .table(Track::Table)
                    .col(Track::Path)
                    .to_owned(),
            )
            .await?;

        // Composite index for artist + album queries (common in music apps)
        manager
            .create_index(
                Index::create()
                    .name("idx_track_artist_album")
                    .table(Track::Table)
                    .col(Track::Artist)
                    .col(Track::Album)
                    .to_owned(),
            )
            .await?;

        // Index on album_artist for album grouping
        manager
            .create_index(
                Index::create()
                    .name("idx_track_album_artist")
                    .table(Track::Table)
                    .col(Track::AlbumArtist)
                    .to_owned(),
            )
            .await?;

        // Index on album for album-based queries
        manager
            .create_index(
                Index::create()
                    .name("idx_track_album")
                    .table(Track::Table)
                    .col(Track::Album)
                    .to_owned(),
            )
            .await?;

        // Index on genre for filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_track_genre")
                    .table(Track::Table)
                    .col(Track::Genre)
                    .to_owned(),
            )
            .await?;

        // Index on year for chronological queries
        manager
            .create_index(
                Index::create()
                    .name("idx_track_year")
                    .table(Track::Table)
                    .col(Track::Year)
                    .to_owned(),
            )
            .await?;

        // Composite index for album track ordering
        manager
            .create_index(
                Index::create()
                    .name("idx_track_album_disc_track")
                    .table(Track::Table)
                    .col(Track::Album)
                    .col(Track::DiscNumber)
                    .col(Track::TrackNumber)
                    .to_owned(),
            )
            .await?;

        // Index on extension for file type filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_track_extension")
                    .table(Track::Table)
                    .col(Track::Extension)
                    .to_owned(),
            )
            .await?;

        Ok(())
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
    DiscNumber,
    TrackNumber,
    Year,
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
    AlbumArtPath,
    AlbumArtMimeType,
    AlbumArtSize,
    Created,
    Modified,
}
