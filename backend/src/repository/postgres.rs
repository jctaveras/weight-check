use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use super::{HardwareRepository, HealthRepository, ModelRepository, RunRepository};
use crate::domain::{
    HardwareProfile, Metric, ModelInfo, NewHardwareProfile, NewModel, NewRun, Run, RunFilter,
};
use crate::error::{AppError, AppResult};

/// PostgreSQL implementation of every repository trait.
#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn db_err(e: sqlx::Error) -> AppError {
    AppError::Database(e.to_string())
}

const MODEL_COLUMNS: &str = "id, name, file_path, architecture, quantization, size_label, \
     block_count, file_size_bytes, created_at";

const RUN_COLUMNS: &str = "id, model_id, hardware_id, category::text AS category, benchmark_name, \
     status::text AS status, config, details, error, started_at, finished_at, created_at";

#[derive(FromRow)]
struct ModelRow {
    id: Uuid,
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
            id: r.id,
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
    id: Uuid,
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
            id: r.id,
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
    id: Uuid,
    model_id: Uuid,
    hardware_id: Uuid,
    category: String,
    benchmark_name: String,
    status: String,
    config: Value,
    details: Option<Value>,
    error: Option<String>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl TryFrom<RunRow> for Run {
    type Error = AppError;

    fn try_from(r: RunRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: r.id,
            model_id: r.model_id,
            hardware_id: r.hardware_id,
            category: r.category.parse().map_err(AppError::Database)?,
            benchmark_name: r.benchmark_name,
            status: r.status.parse().map_err(AppError::Database)?,
            config: r.config,
            details: r.details,
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
impl ModelRepository for PgStore {
    async fn upsert(&self, m: NewModel) -> AppResult<ModelInfo> {
        let sql = format!(
            "INSERT INTO models (id, name, file_path, architecture, quantization, size_label, \
                                 block_count, file_size_bytes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (file_path) DO UPDATE SET
                 name = EXCLUDED.name,
                 architecture = EXCLUDED.architecture,
                 quantization = EXCLUDED.quantization,
                 size_label = EXCLUDED.size_label,
                 block_count = EXCLUDED.block_count,
                 file_size_bytes = EXCLUDED.file_size_bytes
             RETURNING {MODEL_COLUMNS}"
        );
        sqlx::query_as::<_, ModelRow>(&sql)
            .bind(Uuid::new_v4())
            .bind(&m.name)
            .bind(&m.file_path)
            .bind(&m.architecture)
            .bind(&m.quantization)
            .bind(&m.size_label)
            .bind(m.block_count)
            .bind(m.file_size_bytes)
            .fetch_one(&self.pool)
            .await
            .map(Into::into)
            .map_err(db_err)
    }

    async fn get(&self, id: Uuid) -> AppResult<Option<ModelInfo>> {
        let sql = format!("SELECT {MODEL_COLUMNS} FROM models WHERE id = $1");
        sqlx::query_as::<_, ModelRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map(|r| r.map(Into::into))
            .map_err(db_err)
    }
}

#[async_trait]
impl HardwareRepository for PgStore {
    async fn upsert(&self, p: NewHardwareProfile) -> AppResult<HardwareProfile> {
        let fingerprint = p.fingerprint();
        sqlx::query_as::<_, HardwareRow>(
            "INSERT INTO hardware_profiles (id, cpu, gpu, vram_total_bytes, ram_total_bytes, \
                                            backend, driver_version, fingerprint)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (fingerprint) DO UPDATE SET driver_version = EXCLUDED.driver_version
             RETURNING id, cpu, gpu, vram_total_bytes, ram_total_bytes, backend, driver_version, \
                       fingerprint",
        )
        .bind(Uuid::new_v4())
        .bind(&p.cpu)
        .bind(&p.gpu)
        .bind(p.vram_total_bytes)
        .bind(p.ram_total_bytes)
        .bind(&p.backend)
        .bind(&p.driver_version)
        .bind(&fingerprint)
        .fetch_one(&self.pool)
        .await
        .map(Into::into)
        .map_err(db_err)
    }
}

#[async_trait]
impl RunRepository for PgStore {
    async fn create(&self, run: NewRun) -> AppResult<Run> {
        let sql = format!(
            "INSERT INTO benchmark_runs (id, model_id, hardware_id, category, benchmark_name, config)
             VALUES ($1, $2, $3, $4::benchmark_category, $5, $6)
             RETURNING {RUN_COLUMNS}"
        );
        sqlx::query_as::<_, RunRow>(&sql)
            .bind(Uuid::new_v4())
            .bind(run.model_id)
            .bind(run.hardware_id)
            .bind(run.category.as_str())
            .bind(&run.benchmark_name)
            .bind(&run.config)
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?
            .try_into()
    }

    async fn mark_running(&self, id: Uuid) -> AppResult<()> {
        sqlx::query(
            "UPDATE benchmark_runs SET status = 'running', started_at = now() WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn complete(&self, id: Uuid, details: Value, metrics: &[Metric]) -> AppResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query(
            "UPDATE benchmark_runs SET status = 'completed', details = $2, finished_at = now()
             WHERE id = $1",
        )
        .bind(id)
        .bind(&details)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;

        if !metrics.is_empty() {
            let mut qb: QueryBuilder<Postgres> =
                QueryBuilder::new("INSERT INTO metrics (run_id, name, value, unit) ");
            qb.push_values(metrics, |mut b, m| {
                b.push_bind(id)
                    .push_bind(&m.name)
                    .push_bind(m.value)
                    .push_bind(&m.unit);
            });
            qb.build().execute(&mut *tx).await.map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn fail(&self, id: Uuid, error: &str) -> AppResult<()> {
        sqlx::query(
            "UPDATE benchmark_runs SET status = 'failed', error = $2, finished_at = now()
             WHERE id = $1",
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn get(&self, id: Uuid) -> AppResult<Option<Run>> {
        let sql = format!("SELECT {RUN_COLUMNS} FROM benchmark_runs WHERE id = $1");
        sqlx::query_as::<_, RunRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .map(TryInto::try_into)
            .transpose()
    }

    async fn list(&self, filter: &RunFilter) -> AppResult<Vec<Run>> {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(format!(
            "SELECT {RUN_COLUMNS} FROM benchmark_runs WHERE TRUE"
        ));
        if let Some(model_id) = filter.model_id {
            qb.push(" AND model_id = ").push_bind(model_id);
        }
        if let Some(category) = filter.category {
            qb.push(" AND category = ")
                .push_bind(category.as_str())
                .push("::benchmark_category");
        }
        if let Some(name) = &filter.benchmark_name {
            qb.push(" AND benchmark_name = ").push_bind(name.clone());
        }
        qb.push(" ORDER BY created_at DESC LIMIT ")
            .push_bind(filter.limit.unwrap_or(100));

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
            "SELECT name, value, unit FROM metrics WHERE run_id = $1 ORDER BY id",
        )
        .bind(run_id)
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
impl HealthRepository for PgStore {
    async fn ping(&self) -> AppResult<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(db_err)
    }
}
