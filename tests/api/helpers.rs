use std::net::TcpListener;
use zero2prod::startup::run;
use sqlx::{PgConnection, PgPool, Connection, Executor};
use zero2prod::configuration::{get_configuration, DatabaseSettings};
use zero2prod::telemetry::{get_subscriber, init_subscriber};
use uuid::Uuid;
use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
use wiremock::MockServer;
use zero2prod::email_client::EmailClient;
use zero2prod::startup::Application;


static TRACING: Lazy<()> = Lazy::new(|| {
    let filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, filter_level, std::io::sink);
        init_subscriber(subscriber);
    };
});


pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub db_pool: PgPool,
    pub email_server: MockServer,
}

pub struct ConfirmationLinks {
    pub plain_text: reqwest::Url,
    pub html: reqwest::Url,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("failed to execute request")
    }

    pub async fn post_newsletters(&self, body: serde_json::Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/newsletters", &app.address))
            .json(&body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();

            assert_eq!(links.len(), 1);

            let mut confirmation_link = reqwest::Url::parse(links[0].as_str()).unwrap();
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html = get_link(&body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(&body["TextBody"].as_str().unwrap());

        ConfirmationLinks {
            plain_text,
            html,
        }
    }
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let _connection = PgConnection::connect(&config.connection_string_wo_db().expose_secret())
        .await
        .expect("failed to connect to postgres")
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("failed to create database");

    let connection_pool = PgPool::connect(&config.connection_string().expose_secret())
        .await
        .expect("failed to connect to postgres");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("failed to migrate the database");

    connection_pool
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let email_server = MockServer::start().await;

    let config = {
        let mut config = get_configuration().expect("failed to read configuration");
        config.database.database_name = Uuid::new_v4().to_string(); // different db for each test
        config.application.port = 0; // random OS port
        config.email_client.base_url = email_server.uri();
        config
    };
    let db_pool = configure_database(&config.database).await; // for test purposes
    let application = Application::build(config).expect("failed to build application");
    let port = application.port(); // actually assigned port by OS
    let address = format!("http://127.0.0.1:{}", port);

    let _ = tokio::spawn(application.run_until_stopped());

    TestApp { address, port, db_pool, email_server }
}