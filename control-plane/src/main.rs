mod background_tasks;
mod db;
mod dispatcher;
mod error;
pub mod github;
mod handlers;
mod middleware;
mod pipeline;
mod types;
mod webhooks;

use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    http::{header, Method},
    routing::{get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use pipeline::Parser;
use tokio::{signal, net::TcpListener};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{db::Database, dispatcher::Dispatcher, error::AppError, github::GitHubClient};

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub dispatcher: Dispatcher,
    pub webhook_secret: String,
    pub pipeline_parser: Parser,
    pub github: Option<GitHubClient>,
}

#[tokio::main]
async fn main() -> Result<(), error::AppError> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "control_plane=debug,axum::rejection=trace,tower_http=info".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Control plane starting up...");

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://glacier:glacier123@localhost/glacier?sslmode=disable".into());

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(25)
        .min_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    let db = Database::new(pool.clone());
    let webhook_secret = env::var("WEBHOOK_SECRET").unwrap_or_default();
    let pipeline_parser = Parser::new();

    let github = env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty()).map(|token| {
        tracing::info!("GitHub commit status reporting enabled");
        GitHubClient::new(token)
    });

    let dispatcher = Dispatcher::new(db.clone(), github.clone());

    let state = Arc::new(AppState {
        db,
        dispatcher,
        webhook_secret,
        pipeline_parser,
        github,
    });

    background_tasks::spawn_all(&state);

    let v3_routes = Router::new()
        .route("/connect", post(handlers::connect_agent))
        .route("/ping", get(handlers::ping))
        .route("/heartbeat", post(handlers::heartbeat))
        .route("/disconnect", post(handlers::disconnect_agent))
        .route("/jobs/:job_id", get(handlers::get_job))
        .route("/jobs/:job_id/accept", put(handlers::accept_job))
        .route("/jobs/:job_id/start", put(handlers::start_job))
        .route("/jobs/:job_id/finish", put(handlers::finish_job))
        .route("/jobs/:job_id/chunks", post(handlers::upload_chunk))
        .route("/jobs/:job_id/data/exists", post(handlers::metadata_exists))
        .route("/jobs/:job_id/data/set", post(handlers::metadata_set))
        .route("/jobs/:job_id/data/get", post(handlers::metadata_get))
        .route("/jobs/:job_id/data/keys", post(handlers::metadata_keys))
        .route("/jobs/:job_id/pipelines", post(handlers::upload_pipeline))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::agent_auth::auth_middleware,
        ));

    let admin_routes = Router::new()
        .route("/tokens", get(handlers::admin_list_tokens))
        .route("/tokens/:token_id", get(handlers::admin_get_token));

    let auth_routes = Router::new()
        .route("/register", post(handlers::user_register))
        .route("/login", post(handlers::user_login))
        .route("/me", get(handlers::user_me))
        .route("/logout", post(handlers::user_logout));

    let org_resource_routes = Router::new()
        .route("/", get(handlers::get_organization))
        .route("/invitations", post(handlers::create_organization_invitation))
        .route("/members/:user_id", put(handlers::update_member_role).delete(handlers::remove_member))
        .route("/builds", get(handlers::list_org_builds))
        .route("/pipelines", get(handlers::list_user_pipelines).post(handlers::create_user_pipeline))
        .route("/pipelines/:pipeline_slug", get(handlers::get_user_pipeline).patch(handlers::update_user_pipeline).delete(handlers::delete_user_pipeline))
        .route("/pipelines/:pipeline_slug/builds", get(handlers::get_pipeline_builds).post(handlers::create_build))
        .route("/pipelines/:pipeline_slug/builds/:number", get(handlers::get_build))
        .route("/pipelines/:pipeline_slug/builds/:number/jobs/:job_id/log", get(handlers::get_job_log))
        .route("/queues", get(handlers::list_user_queues).post(handlers::create_user_queue))
        .route("/queues/:id", get(handlers::get_user_queue).put(handlers::update_user_queue).delete(handlers::delete_user_queue))
        .route("/agent-tokens", get(handlers::list_user_agent_tokens).post(handlers::create_user_agent_token))
        .route("/agent-tokens/:id", get(handlers::get_user_agent_token).delete(handlers::delete_user_agent_token))
        .route("/agents", get(handlers::list_user_agents));

    let org_routes = Router::new()
        .route("/", get(handlers::list_organizations).post(handlers::create_organization))
        .route("/join/:token", post(handlers::join_organization))
        .nest("/:org_slug", org_resource_routes);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .expose_headers([
            header::LINK,
            axum::http::HeaderName::from_static("x-total-count"),
        ]);

    let router = Router::new()
        .route("/v3/register", post(handlers::register_agent))
        .nest("/v3", v3_routes)
        .nest("/api/admin", admin_routes)
        .nest("/api/auth", auth_routes)
        .nest("/api/v2/organizations", org_routes)
        .route("/api/v2/builds", get(handlers::list_all_builds))
        .route("/webhooks/github/:secret", post(webhooks::handle_github))
        .route(
            "/api/health",
            get(|| async { axum::response::IntoResponse::into_response("OK") }),
        )
        .with_state(state.clone())
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(80);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting server on :{}", port);

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| AppError::Message(e.to_string()))?;
    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| AppError::Message(e.to_string()))?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
