use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};
use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use secrecy::ExposeSecret;
use zero2prod::email_client::EmailClient;


#[actix_web::main]
async fn main() -> std::io::Result<()> {
	let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
	init_subscriber(subscriber);

	let configuration = get_configuration().expect("fail to read cofiguration");

	let connection_pool = PgPoolOptions::new()
		.connect_lazy(&configuration.database.connection_string().expose_secret())
		.expect("failed to create postgres connection pool");

	let sender_email = configuration.email_client
		.sender()
		.expect("invalid sender email address");
	let timeout = configuration.email_client.timeout();
	let base_url = configuration.email_client.base_url;
	let authorization_token = configuration.email_client.authorization_token;
	let email_client = EmailClient::new(base_url, sender_email, authorization_token, timeout);

	let address = format!("{}:{}", configuration.application.host, configuration.application.port);
	let listener = TcpListener::bind(address).expect("failed to bind a random adress");

    run(listener, connection_pool, email_client)?.await
}
