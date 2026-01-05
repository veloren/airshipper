use std::{collections::HashMap, io::ErrorKind};

use chrono::{DateTime, Utc};
use sqlx::{Any, Executor, QueryBuilder, Row, Transaction};

use crate::{
    FsStorage,
    db::Db,
    error::{ProcessError, ServerError},
    models::Artifact,
};

#[tracing::instrument(skip(db))]
pub async fn any_artifacts_exist(db: &Db, cmp: &[Artifact]) -> Result<bool, ServerError> {
    let uris = cmp.iter().map(|x| &x.download_uri);

    let mut query_builder = QueryBuilder::new(
        r"SELECT COUNT(id) as cnt
        FROM artifacts
        WHERE (download_uri) IN ",
    );
    query_builder.push_tuples(uris, |mut b, uri| {
        b.push_bind(uri);
    });

    let count: i64 = query_builder
        .build_query_scalar()
        .fetch_one(&db.pool)
        .await?;

    Ok(count > 0)
}

#[tracing::instrument(skip(db))]
pub async fn insert_artifact(db: &Db, artifact: &Artifact) -> Result<i64, ProcessError> {
    let query = sqlx::query_scalar(
        r"INSERT INTO artifacts (build_id, date, hash, author, merged_by, os, arch, channel, file_name, download_uri) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    ).bind(artifact.build_id)
    .bind(artifact.date.to_utc().to_rfc3339())
    .bind(&artifact.hash)
    .bind(&artifact.author)
    .bind(&artifact.merged_by)
    .bind(&artifact.os)
    .bind(&artifact.arch)
    .bind(&artifact.channel)
    .bind(&artifact.file_name)
    .bind(&artifact.download_uri);

    let id: i64 = query.fetch_one(&db.pool).await?;
    Ok(id)
}

pub struct VersionUri {
    pub version: String,
    pub uri: String,
}

#[tracing::instrument(skip(db))]
pub async fn get_latest_version_uri<
    T: ToString + std::fmt::Debug,
    U: ToString + std::fmt::Debug,
    Y: ToString + std::fmt::Debug,
>(
    db: &Db,
    searched_os: T,
    searched_arch: U,
    searched_channel: Y,
) -> Result<Option<VersionUri>, ServerError> {
    let searched_os = searched_os.to_string().to_lowercase();
    let searched_arch = searched_arch.to_string().to_lowercase();
    let searched_channel = searched_channel.to_string().to_lowercase();

    let query = sqlx::query(
        r"SELECT hash, download_uri
          FROM artifacts
          WHERE os = ? AND arch = ? AND channel = ?
          ORDER BY date DESC
          LIMIT 1;",
    )
    .bind(&searched_os)
    .bind(&searched_arch)
    .bind(&searched_channel);

    let row = db.pool.fetch_optional(query).await?;
    match row {
        Some(row) => Ok(Some(VersionUri {
            version: row.try_get("hash")?,
            uri: row.try_get("download_uri")?,
        })),
        None => Ok(None),
    }
}

/// Prunes local db and S3 storage from old nightlies by removing all artifacts but one
/// per os/arch/channel combination
// The urge to put this is a single query might be high, hear me out:
//  - The algorithm should take account of timezones (something that sqlite cannot do)
//    (optional) (NOTE: get_latest_version_uri still depends on it)
//  - The algorithm should not rely on that IDs are always increasing
//  - The algorithm should not rely on implementation behavior (e.g. that GROUP BY returns
//    the first it finds), if its not in the specification.
//  - test it with all suported DB backends
//  - the pruning procedure should not leave dangling files in either the DB or the
//    filesystem, if it can only be deleted in one place but not the other because of
//    (temporary) errors
// Good luck
#[tracing::instrument(skip(db))]
pub async fn prune(db: &Db) -> Result<(), ServerError> {
    let mut con = db.pool.begin().await?;

    let artifacts = artifacts_to_be_pruned(&mut con).await?;

    for (id, file) in artifacts {
        tracing::info!("Deleting prunable artifact: {:?}", file);
        // dont fail on Err here
        match FsStorage::delete_file(&file).await {
            Ok(()) => delete_artifact(&mut con, id).await?,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                delete_artifact(&mut con, id).await?
            },
            _ => (),
        }
    }

    con.commit().await?;

    Ok(())
}

async fn artifacts_to_be_pruned(
    con: &mut Transaction<'static, Any>,
) -> Result<impl Iterator<Item = (i64, String)> + use<>, ServerError> {
    // Currently date is a STRING and the order DESC might cause weird effects IF we
    // would store different timezones.
    let query =
        sqlx::query("SELECT id, date, file_name, channel, os, arch FROM artifacts");
    let rows = con.fetch_all(query).await?;

    #[derive(PartialEq, Eq, Hash)]
    struct ArtifactGroup {
        os: String,
        arch: String,
        channel: String,
    }

    struct Artifact {
        id: i64,
        date: DateTime<Utc>,
        file_name: String,
    }

    let mut artifacts: HashMap<ArtifactGroup, Vec<Artifact>> = HashMap::new();

    for row in rows {
        let date: String = row.try_get("date")?;
        // Old database format:
        //   2024-09-05 16:56:55 UTC
        // new Format:
        //   rfc3339
        let date = DateTime::parse_from_rfc3339(&date)
            .or_else(|_| {
                DateTime::parse_from_str(
                    &date.replace(" UTC", " +0000"),
                    "%Y-%m-%d %H:%M:%S %z",
                )
            })?
            .to_utc();

        let group = ArtifactGroup {
            os: row.try_get("os")?,
            arch: row.try_get("arch")?,
            channel: row.try_get("channel")?,
        };
        let artifact = Artifact {
            id: row.try_get("id")?,
            date,
            file_name: row.try_get("file_name")?,
        };

        let grouped = artifacts.entry(group).or_default();
        grouped.push(artifact);
    }

    for (_, grouped) in artifacts.iter_mut() {
        grouped.sort_by_key(|e| e.date);
        // last element is newest, so we pop it to keep it
        grouped.pop();
    }

    Ok(artifacts
        .into_values()
        .flat_map(|grouped| grouped.into_iter().map(|e| (e.id, e.file_name))))
}

/// deletes single artifact from db
async fn delete_artifact(
    con: &mut Transaction<'static, Any>,
    id: i64,
) -> Result<(), ServerError> {
    let query = sqlx::query("DELETE FROM artifacts WHERE id = ?").bind(id);
    let _rows = con.execute(query).await?;
    Ok(())
}
