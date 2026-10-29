use std::path::Path;
use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use sqlx::types::Json;
use sqlx::{FromRow, QueryBuilder, Sqlite};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use super::{HardwareRepository, HealthRepository, ModelRepository, RunRepository};
use crate::domain::{
    HardwareProfile, Metric, ModelInfo, NewHardwareProfile, NewModel, NewRun, Run, RunFilter,
};
use crate::error::{AppError, AppResult};

/// Embedded SQLite implementation of every repository trait.
///
/// UUIDs are stored as hyphenated TEXT, timestamps as RFC 3339 TEXT written from
/// Rust, and JSON documents as TEXT.
#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Opens (creating if missing) the database at `url`, enables foreign keys and
    /// WAL journaling, and applies the embedded migrations.
    pub async fn connect(url: &str) -> AppResult<Self> {
        Self::connect_with(SqliteConnectOptions::from_str(url).map_err(db_err)?).await
    }

    /// Like [`SqliteStore::connect`], for a database file path. Unlike a URL, the path
    /// needs no escaping, so it suits OS data directories that contain spaces.
    pub async fn open(path: &Path) -> AppResult<Self> {
        Self::connect_with(SqliteConnectOptions::new().filename(path)).await
    }

    async fn connect_with(options: SqliteConnectOptions) -> AppResult<Self> {
        let options = options
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(db_err)?;
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(Self::new(pool))
    }
}

fn db_err(e: sqlx::Error) -> AppError {
    AppError::Database(e.to_string())
}

const MODEL_COLUMNS: &str = "id, name, file_path, architecture, quantization, size_label, \
     block_count, file_size_bytes, created_at";

const RUN_COLUMNS: &str = "id, model_id, hardware_id, category, benchmark_name, status, config, \
     details, error, started_at, finished_at, created_at";

/// Runs that may still change status. Once finished (completed, failed, cancelled) a run's
/// status is final.
const UNFINISHED: &str = "status IN ('queued', 'running')";

#[derive(FromRow)]
struct ModelRow {
    id: Hyphenated,
    name: String,
    file_path: String,
    architecture: Option<String>,
    quantization: Option<String>,
    size_label: Option<String>,
    block_count: Option<i32>,
    file_size_bytes: i64,
    created_at: DateTime<Utc>,
}

impl From<ModelRow> for ModelInfo {
    fn from(r: ModelRow) -> Self {
        Self {
            id: r.id.into_uuid(),
            name: r.name,
            file_path: r.file_path,
            architecture: r.architecture,
            quantization: r.quantization,
            size_label: r.size_label,
            block_count: r.block_count,
            file_size_bytes: r.file_size_bytes,
            created_at: r.created_at,
        }
    }
}

#[derive(FromRow)]
struct HardwareRow {
    id: Hyphenated,
    cpu: String,
    gpu: Option<String>,
    vram_total_bytes: Option<i64>,
    ram_total_bytes: i64,
    backend: String,
    driver_version: Option<String>,
    fingerprint: String,
}

impl From<HardwareRow> for HardwareProfile {
    fn from(r: HardwareRow) -> Self {
        Self {
            id: r.id.into_uuid(),
            cpu: r.cpu,
            gpu: r.gpu,
            vram_total_bytes: r.vram_total_bytes,
            ram_total_bytes: r.ram_total_bytes,
            backend: r.backend,
            driver_version: r.driver_version,
            fingerprint: r.fingerprint,
        }
    }
}

#[derive(FromRow)]
struct RunRow {
    id: Hyphenated,
    model_id: Hyphenated,
    hardware_id: Hyphenated,
    category: String,
    benchmark_name: String,
    status: String,
    config: Json<Value>,
    details: Option<Json<Value>>,
    error: Option<String>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl TryFrom<RunRow> for Run {
    type Error = AppError;

    fn try_from(r: RunRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: r.id.into_uuid(),
            model_id: r.model_id.into_uuid(),
            hardware_id: r.hardware_id.into_uuid(),
            category: r.category.parse().map_err(AppError::Database)?,
            benchmark_name: r.benchmark_name,
            status: r.status.parse().map_err(AppError::Database)?,
            config: r.config.0,
            details: r.details.map(|d| d.0),
            error: r.error,
            started_at: r.started_at,
            finished_at: r.finished_at,
            created_at: r.created_at,
        })
    }
}

#[derive(FromRow)]
struct MetricRow {
    name: String,
    value: f64,
    unit: String,
}

#[async_trait]
impl ModelRepository for SqliteStore {
    async fn upsert(&self, m: NewModel) -> AppResult<ModelInfo> {
        let sql = format!(
            "INSERT INTO models (id, name, file_path, architecture, quantization, size_label, \
                                 block_count, file_size_bytes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT (file_path) DO UPDATE SET
                 name = excluded.name,
                 architecture = excluded.architecture,
                 quantization = excluded.quantization,
                 size_label = excluded.size_label,
                 block_count = excluded.block_count,
                 file_size_bytes = excluded.file_size_bytes
             RETURNING {MODEL_COLUMNS}"
        );
        sqlx::query_as::<_, ModelRow>(&sql)
            .bind(Uuid::new_v4().hyphenated())
            .bind(&m.name)
            .bind(&m.file_path)
            .bind(&m.architecture)
            .bind(&m.quantization)
            .bind(&m.size_label)
            .bind(m.block_count)
            .bind(m.file_size_bytes)
            .bind(Utc::now())
            .fetch_one(&self.pool)
            .await
            .map(Into::into)
            .map_err(db_err)
    }

    async fn get(&self, id: Uuid) -> AppResult<Option<ModelInfo>> {
        let sql = format!("SELECT {MODEL_COLUMNS} FROM models WHERE id = ?1");
        sqlx::query_as::<_, ModelRow>(&sql)
            .bind(id.hyphenated())
            .fetch_optional(&self.pool)
            .await
            .map(|r| r.map(Into::into))
            .map_err(db_err)
    }
}

#[async_trait]
impl HardwareRepository for SqliteStore {
    async fn upsert(&self, p: NewHardwareProfile) -> AppResult<HardwareProfile> {
        let fingerprint = p.fingerprint();
        sqlx::query_as::<_, HardwareRow>(
            "INSERT INTO hardware_profiles (id, cpu, gpu, vram_total_bytes, ram_total_bytes, \
                                            backend, driver_version, fingerprint, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT (fingerprint) DO UPDATE SET driver_version = excluded.driver_version
             RETURNING id, cpu, gpu, vram_total_bytes, ram_total_bytes, backend, driver_version, \
                       fingerprint",
        )
        .bind(Uuid::new_v4().hyphenated())
        .bind(&p.cpu)
        .bind(&p.gpu)
        .bind(p.vram_total_bytes)
        .bind(p.ram_total_bytes)
        .bind(&p.backend)
        .bind(&p.driver_version)
        .bind(&fingerprint)
        .bind(Utc::now())
        .fetch_one(&self.pool)
        .await
        .map(Into::into)
        .map_err(db_err)
    }
}

#[async_trait]
impl RunRepository for SqliteStore {
    async fn create(&self, run: NewRun) -> AppResult<Run> {
        let sql = format!(
            "INSERT INTO benchmark_runs (id, model_id, hardware_id, category, benchmark_name, \
                                         config, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             RETURNING {RUN_COLUMNS}"
        );
        sqlx::query_as::<_, RunRow>(&sql)
            .bind(Uuid::new_v4().hyphenated())
            .bind(run.model_id.hyphenated())
            .bind(run.hardware_id.hyphenated())
            .bind(run.category.as_str())
            .bind(&run.benchmark_name)
            .bind(Json(&run.config))
            .bind(Utc::now())
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?
            .try_into()
    }

    async fn mark_running(&self, id: Uuid) -> AppResult<()> {
        // A run cancelled while queued stays cancelled.
        sqlx::query(
            "UPDATE benchmark_runs SET status = 'running', started_at = ?2
             WHERE id = ?1 AND status = 'queued'",
        )
        .bind(id.hyphenated())
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn complete(&self, id: Uuid, details: Value, metrics: &[Metric]) -> AppResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        // Only an unfinished run completes: a run cancelled meanwhile keeps that status and
        // gets no metrics.
        let updated = sqlx::query(&format!(
            "UPDATE benchmark_runs SET status = 'completed', details = ?2, finished_at = ?3
             WHERE id = ?1 AND {UNFINISHED}"
        ))
        .bind(id.hyphenated())
        .bind(Json(&details))
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();

        if updated > 0 && !metrics.is_empty() {
            let mut qb: QueryBuilder<Sqlite> =
                QueryBuilder::new("INSERT INTO metrics (run_id, name, value, unit) ");
            qb.push_values(metrics, |mut b, m| {
                b.push_bind(id.hyphenated())
                    .push_bind(&m.name)
                    .push_bind(m.value)
                    .push_bind(&m.unit);
            });
            qb.build().execute(&mut *tx).await.map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn fail(&self, id: Uuid, error: &str) -> AppResult<()> {
        sqlx::query(&format!(
            "UPDATE benchmark_runs SET status = 'failed', error = ?2, finished_at = ?3
             WHERE id = ?1 AND {UNFINISHED}"
        ))
        .bind(id.hyphenated())
        .bind(error)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn cancel(&self, id: Uuid) -> AppResult<bool> {
        let updated = sqlx::query(&format!(
            "UPDATE benchmark_runs SET status = 'cancelled', finished_at = ?2
             WHERE id = ?1 AND {UNFINISHED}"
        ))
        .bind(id.hyphenated())
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        Ok(updated > 0)
    }

    async fn delete(&self, id: Uuid) -> AppResult<bool> {
        let deleted = sqlx::query("DELETE FROM benchmark_runs WHERE id = ?1")
            .bind(id.hyphenated())
            .execute(&self.pool)
            .await
            .map_err(db_err)?
            .rows_affected();
        Ok(deleted > 0)
    }

    async fn get(&self, id: Uuid) -> AppResult<Option<Run>> {
        let sql = format!("SELECT {RUN_COLUMNS} FROM benchmark_runs WHERE id = ?1");
        sqlx::query_as::<_, RunRow>(&sql)
            .bind(id.hyphenated())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .map(TryInto::try_into)
            .transpose()
    }

    async fn list(&self, filter: &RunFilter) -> AppResult<Vec<Run>> {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(format!(
            "SELECT {RUN_COLUMNS} FROM benchmark_runs WHERE 1 = 1"
        ));
        if let Some(model_id) = filter.model_id {
            qb.push(" AND model_id = ").push_bind(model_id.hyphenated());
        }
        if let Some(category) = filter.category {
            qb.push(" AND category = ").push_bind(category.as_str());
        }
        if let Some(name) = &filter.benchmark_name {
            qb.push(" AND benchmark_name = ").push_bind(name.clone());
        }
        if let Some(status) = filter.status {
            qb.push(" AND status = ").push_bind(status.as_str());
        }
        // rowid breaks ties between runs created within the same clock tick.
        qb.push(" ORDER BY created_at DESC, rowid DESC LIMIT ")
            .push_bind(filter.limit.unwrap_or(100))
            .push(" OFFSET ")
            .push_bind(filter.offset.unwrap_or(0));

        qb.build_query_as::<RunRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    async fn metrics(&self, run_id: Uuid) -> AppResult<Vec<Metric>> {
        sqlx::query_as::<_, MetricRow>(
            "SELECT name, value, unit FROM metrics WHERE run_id = ?1 ORDER BY id",
        )
        .bind(run_id.hyphenated())
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|r| Metric::new(r.name, r.value, r.unit))
                .collect()
        })
        .map_err(db_err)
    }
}

#[async_trait]
impl HealthRepository for SqliteStore {
    async fn ping(&self) -> AppResult<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(db_err)
    }
}
