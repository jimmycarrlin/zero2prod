use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};
use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use secrecy::ExposeSecret;
use zero2prod::email_client::EmailClient;
use zero2prod::startup::Application;


#[actix_web::main]
async fn main() -> std::io::Result<()> {
	let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
	init_subscriber(subscriber);

	let configuration = get_configuration().expect("fail to read configuration");

	let application = Application::build(configuration)?;
	application.run_until_stopped().await?;

	Ok(())
}
