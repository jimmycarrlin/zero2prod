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

async fn spawn_app() -> TestApp {
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

async fn configure_database(config: &DatabaseSettings) -> PgPool {
	let mut connection = PgConnection::connect(&config.connection_string_wo_db().expose_secret())
		.await
		.expect("failde to connect to postgres")
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


#[actix_rt::test]
async fn health_check_works() {
	let app = spawn_app().await;

	let client = reqwest::Client::new();
	let response = client
		.get(format!("{}/health_check", &app.address))
		.send()
		.await
		.expect("failed to execute request");

	assert!(response.status().is_success());
	assert_eq!(Some(0), response.content_length());
}

#[actix_rt::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(format!("{}/subscriptions", &app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
		.send()
        .await
        .expect("failed to execute request.");

    assert_eq!(200, response.status().as_u16());

	let saved = sqlx::query!("SELECT email, name from SUBSCRIPTIONS")
		.fetch_one(&app.db_pool)
		.await
		.expect("failed to fetch saved subscriptions");

	assert_eq!(saved.email, "ursula_le_guin@gmail.com");
	assert_eq!(saved.name, "le guin");
}

#[actix_rt::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email")
    ];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(format!("{}/subscriptions", &app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("failed to execute request.");

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

#[tokio::test]
async fn subscribe_returns_a_400_when_fields_are_present_but_empty() {
	let app = spawn_app().await;
	let client = reqwest::Client::new();
	let test_cases = vec![
		("name=&email=ursula_le_guin%40gmail.com", "empty name"),
		("name=Ursula&email=", "empty email"),
		("name=Ursula&email=definitely-not-an-email", "invalid email"),
	];

	for (body, description) in test_cases {
		let response = client
			.post(format!("{}/subscriptions", &app.address))
			.header("Content-Type", "application/x-www-form-urlencoded")
			.body(body)
			.send()
			.await
			.expect("Failed to execute request.");

		assert_eq!(
			400,
			response.status().as_u16(),
			"The API did not return a 400 Bad Request when the payload was {}.",
			description
		);
	}
}
