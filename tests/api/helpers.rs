use std::net::TcpListener;
use zero2prod::startup::run;
use sqlx::{PgConnection, PgPool, Connection, Executor};
use zero2prod::configuration::{get_configuration, DatabaseSettings};
use zero2prod::telemetry::{get_subscriber, init_subscriber};
use uuid::Uuid;
use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
use zero2prod::email_client::EmailClient;


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

    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let mut configuration = get_configuration().expect("failed to read configuration");
    configuration.database.database_name = Uuid::new_v4().to_string();
    let connection_pool = configure_database(&configuration.database).await;

    let sender_email = configuration.email_client
        .sender()
        .expect("invalid sender email address");
    let timeout = configuration.email_client.timeout();
    let base_url = configuration.email_client.base_url;
    let authorization_token = configuration.email_client.authorization_token;
    let email_client = EmailClient::new(base_url, sender_email, authorization_token, timeout);

    let server = run(listener, connection_pool.clone(), email_client)
        .expect("failed to bind address");
    let _ = tokio::spawn(server);

    TestApp {
        address,
        db_pool: connection_pool
    }
}