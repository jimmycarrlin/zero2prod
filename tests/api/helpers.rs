use std::net::TcpListener;
use zero2prod::startup::run;
use sqlx::{PgConnection, PgPool, Connection, Executor};
use zero2prod::configuration::{get_configuration, DatabaseSettings};
use zero2prod::telemetry::{get_subscriber, init_subscriber};
use uuid::Uuid;
use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
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
    pub db_pool: PgPool,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Request {
        reqwest::Client::new()
            .post(format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("failed to execute request");
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

    let config = {
        let mut config = get_configuration().expect("failed to read configuration");
        config.database.database_name = Uuid::new_v4().to_string(); // different db for each test
        config.application.port = 0; // random OS port
        config
    };
    let db_pool = configure_database(&config.database).await; // for test purposes
    let application = Application::build(config).expect("failed to build application");
    let address = format!("http://127.0.0.1:{}", application.port()); // actually assigned port

    let _ = tokio::spawn(application.run_until_stopped());

    TestApp { address, db_pool }
}