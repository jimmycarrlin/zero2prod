use config::Config;
use secrecy::Secret;
use secrecy::ExposeSecret;
use crate::email_client::EmailClient;
use crate::domain::SubscriberEmail;


pub enum Environment {
	Local,
	Production,
}

#[derive(serde::Deserialize)]
pub struct Settings {
	pub database: DatabaseSettings,
    pub application: ApplicationSettings,
	pub email_client: EmailClientSettings,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    pub port: u16,
    pub host: String,
    pub database_name: String,
}

#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
	pub port: u16,
	pub host: String,
}

#[derive(serde::Deserialize)]
pub struct EmailClientSettings {
	pub base_url: String,
	pub sender_email: String,
	pub authorization_token: Secret<String>,
	pub timeout_milliseconds: u64,
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
	let mut settings = Config::default();
	let config_dir = std::env::current_dir()
		.expect("failed to determine the current directory")
		.join("configuration");

	settings.merge(config::File::from(config_dir.join("base")))?;

	let environment: Environment = std::env::var("APP_ENVIRONMENT")
		.unwrap_or_else(|_| "local".into())
		.try_into()
		.expect("failed to parse APP_ENVIRONMENT");
	settings.merge(config::File::from(config_dir.join(environment.as_str())))?;

	settings.try_deserialize()
}

impl Environment {
	pub fn as_str(&self) -> &'static str {
		match self {
			Environment::Local => "local",
			Environment::Production => "production",
		}
	}
}

impl TryFrom<String> for Environment {
	type Error = String;

	fn try_from(s: String) -> Result<Self, Self::Error> {
		match s.to_lowercase().as_str() {
			"local" => Ok(Environment::Local),
			"production" => Ok(Environment::Production),
			other => Err(format!("{} is not a supported environment, use either `local` or `production`", other)),
		}
	}
}

impl DatabaseSettings {
    pub fn connection_string(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username, self.password.expose_secret(), self.host, self.port, self.database_name
        ))
    }
	pub fn connection_string_wo_db(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}/",
            self.username, self.password.expose_secret(), self.host, self.port
        ))
    }
}

impl EmailClientSettings {
	pub fn sender(&self) -> Result<SubscriberEmail, String> {
		SubscriberEmail::parse(self.sender_email.clone())
	}

	pub fn timeout(&self) -> std::time::Duration {
		std::time::Duration::from_millis(self.timeout_milliseconds)
	}
}