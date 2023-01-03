use actix_web::{web, App, HttpServer, dev::Server};
use sqlx::PgPool;
use std::io;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;
use sqlx::postgres::PgPoolOptions;
use secrecy::{Secret, ExposeSecret};
use crate::email_client::EmailClient;
use crate::configuration::Settings;

use crate::routes::{home, confirm, health_check, publish_newsletter, subscribe, login_form, login};


pub struct Application {
	port: u16,
	server: Server,
}

pub struct ApplicationBaseUrl(pub String);

#[derive(Clone, serde::Deserialize)]
pub struct HmacSecret(pub Secret<String>);


pub fn run(
	listener: TcpListener,
	db_pool: PgPool,
	email_client: EmailClient,
	base_url: String,
	hmac_secret: HmacSecret,
) -> io::Result<Server> {
	let db_pool = web::Data::new(db_pool);
	let email_client = web::Data::new(email_client);
	let base_url = web::Data::new(ApplicationBaseUrl(base_url));
	let hmac_secret = web::Data::new(hmac_secret);

    let server = HttpServer::new(move || {
        App::new()
			.wrap(TracingLogger::default())
			.route("/", web::get().to(home))
            .route("/health_check", web::get().to(health_check))
			.route("/subscriptions", web::post().to(subscribe))
			.route("/subscriptions/confirm", web::get().to(confirm))
			.route("/newsletters", web::post().to(publish_newsletter))
			.route("/login", web::get().to(login_form))
			.route("/login", web::post().to(login))
			.app_data(db_pool.clone())
			.app_data(email_client.clone())
			.app_data(base_url.clone())
			.app_data(hmac_secret.clone())
    })
    .listen(listener)?
    .run();

	Ok(server)
}

impl Application {
	pub fn build(configuration: Settings) -> io::Result<Self> {
		let connection_pool = PgPoolOptions::new()
			.connect_lazy(&configuration.database.connection_string().expose_secret())
			.expect("failed to create postgres connection pool");

		let email_client = {
			let sender_email = configuration.email_client
				.sender()
				.expect("invalid sender email address");
			let timeout = configuration.email_client
				.timeout();
			let base_url = configuration.email_client.base_url;
			let authorization_token = configuration.email_client.authorization_token;
			EmailClient::new(base_url, sender_email, authorization_token, timeout)
		};

		let listener = {
			let host = configuration.application.host;
			let port = configuration.application.port;
			let address = format!("{}:{}", host, port);
			TcpListener::bind(address).expect("failed to bind a random address")
		};

		let port = listener.local_addr().unwrap().port(); // actually assigned port
		let server = run(
			listener,
			connection_pool,
			email_client,
			configuration.application.base_url,
			configuration.application.hmac_secret,
		)?;

		Ok(Self { port, server })
	}

	pub fn port(&self) -> u16 {
		self.port
	}

	pub async fn run_until_stopped(self) -> io::Result<()> {
		self.server.await
	}
}
