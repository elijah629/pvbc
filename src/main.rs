//! GET  /       -> new uuid
//! GET  /{uuid} -> shields.io

use std::collections::HashMap;

use axum::{
    Router,
    extract::{FromRef, FromRequestParts, Path, Query},
    http::{Response, StatusCode, request::Parts},
    response::IntoResponse,
    routing::get,
};
use uuid::Uuid;

use shields::builder::Badge;

use bb8::{Pool, PooledConnection};
use bb8_postgres::PostgresConnectionManager;
use tokio_postgres::NoTls;

// Taken from https://github.com/tokio-rs/axum/blob/main/examples/tokio-postgres/src/main.rs
type ConnectionPool = Pool<PostgresConnectionManager<NoTls>>;

// also stolen
/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

// stolen once more!
// we can also write a custom extractor that grabs a connection from the pool
// which setup is appropriate depends on your application
struct DatabaseConnection(PooledConnection<'static, PostgresConnectionManager<NoTls>>);

impl<S> FromRequestParts<S> for DatabaseConnection
where
    ConnectionPool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = ConnectionPool::from_ref(state);

        let conn = pool.get_owned().await.map_err(internal_error)?;

        Ok(Self(conn))
    }
}

fn badge_style_from_string(s: &str) -> Option<shields::BadgeStyle> {
    match s {
        "flat-square" => Some(shields::BadgeStyle::FlatSquare),
        "plastic" => Some(shields::BadgeStyle::Plastic),
        "for-the-badge" => Some(shields::BadgeStyle::ForTheBadge),
        "social" => Some(shields::BadgeStyle::Social),
        "flat" => Some(shields::BadgeStyle::Flat),
        _ => None,
    }
}

async fn new_uuid(
    DatabaseConnection(conn): DatabaseConnection,
) -> Result<String, (StatusCode, String)> {
    let uuid: Uuid = conn
        .query_one(
            "INSERT INTO counts (id, count) VALUES (gen_random_uuid(), 0) RETURNING id",
            &[],
        )
        .await
        .map_err(internal_error)?
        .get("id");

    Ok(format!(
        r#"Welcome! This is a simple API for generating visitor count badges using shields.io.

Your new unique ID is: {0}
To begin tracking, visit: /{0}

You can customize the badge appearance using any of the query parameters supported by shields.io static badges:
https://shields.io/badges/static-badge

Note: Only query parameters are supported.
      `logoSize`, `cacheSeconds`, and `link` are not supported.
      The default value for label is "visitors""#,
        uuid.to_string()
    ))
}

async fn get_badge(
    Path(uuid): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
    DatabaseConnection(conn): DatabaseConnection,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let result = conn
        .query_opt(
            r#"
            UPDATE counts
            SET count = count + 1
            WHERE id = $1
            RETURNING count
            "#,
            &[&uuid],
        )
        .await
        .map_err(internal_error)?;

    let current_count: i64 = match result {
        Some(row) => row.get("count"),
        None => {
            return Err((StatusCode::NOT_FOUND, "UUID not found".to_string()));
        }
    };

    let current_count = current_count.to_string();

    let badge_style = params
        .get("style")
        .and_then(|badge| badge_style_from_string(badge))
        .unwrap_or(shields::BadgeStyle::Flat);

    let mut badge = Badge::style(badge_style);

    badge.label(
        params
            .get("label")
            .map(|x| x.as_str())
            .unwrap_or("visitors"),
    );

    badge.message(&current_count);

    if let Some(logo) = params.get("logo") {
        badge.logo(logo);
    }

    if let Some(logo_color) = params.get("logoColor") {
        badge.logo_color(logo_color);
    }

    if let Some(label_color) = params.get("labelColor") {
        badge.label_color(label_color);
    }

    if let Some(message_color) = params.get("color").or(params.get("messageColor")) {
        badge.message_color(message_color);
    }

    Response::builder()
        .header("Content-Type", "image/svg+xml")
        .body(badge.build())
        .map_err(internal_error)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use dotenvy::EnvLoader;

    let env = EnvLoader::new().load()?;

    println!("(1/3) Connecting...");
    let manager = PostgresConnectionManager::new_from_stringlike(
        &env.var("POSTGRESQL_CONNECTION_URL")?,
        NoTls,
    )
    .unwrap();

    let pool = Pool::builder().build(manager).await.unwrap();

    println!("(2/3) Init DB");
    {
        println!("  (1/2) Connecting to pool");
        let conn = pool.get().await?;
        println!("  (2/2) Executing init");
        conn.execute(
            r#"
          CREATE TABLE IF NOT EXISTS counts (
                id UUID PRIMARY KEY,
                count BIGINT NOT NULL DEFAULT 0
          );"#,
            &[],
        )
        .await?;
        // drop db
    }

    println!("(3/3) Starting app");
    let app = Router::new()
        .route("/{uuid}", get(get_badge))
        .route("/", get(new_uuid))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind(env.var("HOST")?)
        .await
        .unwrap();
    println!("Ready");

    axum::serve(listener, app).await?;

    Ok(())
}
