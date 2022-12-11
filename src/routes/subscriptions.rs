use std::fmt::{Debug, Display, Formatter};
use actix_web::{web, HttpResponse, http::StatusCode};
use sqlx::{PgPool, Transaction, Postgres};
use uuid::Uuid;
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
use crate::startup::ApplicationBaseUrl;


#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

impl TryFrom<FormData> for NewSubscriber {
	type Error = String;

	fn try_from(value: FormData) -> Result<Self, Self::Error> {
		let name = SubscriberName::parse(value.name)?;
		let email = SubscriberEmail::parse(value.email)?;
		Ok(Self { name, email })
	}
}


#[derive(Debug)]
pub enum SubscribeError {
	ValidationError(String),
	DatabaseError(sqlx::Error),
	StoreTokenError(StoreTokenError),
	SendEmailError(reqwest::Error),
}

impl Display for SubscribeError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "Failed to create new subscriber")
	}
}

impl std::error::Error for SubscribeError {}

impl actix_web::ResponseError for SubscribeError {
	fn status_code(&self) -> StatusCode {
		match self {
			SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
			SubscribeError::DatabaseError(_)
			| SubscribeError::StoreTokenError(_)
			| SubscribeError::SendEmailError(_) => StatusCode::INTERNAL_SERVER_ERROR,
		}
	}
}

impl From<reqwest::Error> for SubscribeError {
	fn from(e: reqwest::Error) -> Self {
		Self::SendEmailError(e)
	}
}

impl From<sqlx::Error> for SubscribeError {
	fn from(e: sqlx::Error) -> Self {
		Self::DatabaseError(e)
	}
}

impl From<StoreTokenError> for SubscribeError {
	fn from(e: StoreTokenError) -> Self {
		Self::StoreTokenError(e)
	}
}

impl From<String> for SubscribeError {
	fn from(e: String) -> Self {
		Self::ValidationError(e)
	}
}


pub struct StoreTokenError(sqlx::Error);

fn error_chain_fmt(
	f: &mut Formatter<'_>,
	e: &impl std::error::Error,
) -> std::fmt::Result {
	writeln!(f, "{}\n", e)?;
	let mut current = e.source();
	while let Some(cause) = current {
		writeln!(f, "Caused by:\n\t{}", cause)?;
		current = cause.source();
	}
	Ok(())
}

impl Debug for StoreTokenError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		error_chain_fmt(f, self)
	}
}

impl Display for StoreTokenError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "A database error was encountered while trying to store a subscription token")
	}
}

impl std::error::Error for StoreTokenError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		Some(&self.0)
	}
}

#[tracing::instrument(
	name = "Adding a new subscriber",
	skip(form, pool, email_client, base_url),
	fields(
		subscriber_email = %form.email,
		subscriber_name = %form.name
	)
)]
pub async fn subscribe(
	form: web::Form<FormData>,
	pool: web::Data<PgPool>,
	email_client: web::Data<EmailClient>,
	base_url: web::Data<ApplicationBaseUrl>
) -> Result<HttpResponse, SubscribeError> {
	let new_subscriber = form.0.try_into()?;

	let mut transaction = pool.begin().await?;
	let new_subscriber_id = insert_subscriber(&new_subscriber, &mut transaction).await?;
	let subscription_token = generate_subscriptions_token();
	store_token(new_subscriber_id, &subscription_token, &mut transaction).await?;
	transaction.commit().await?;

	send_confirmation_email(&email_client, new_subscriber, &base_url.0, &subscription_token).await?;

	Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
	name = "Saving new subscriber details in the database",
	skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
	new_subscriber: &NewSubscriber,
	transaction: &mut Transaction<'_, Postgres>,
) -> Result<Uuid, sqlx::Error> {
	let subscriber_id = Uuid::new_v4();

	sqlx::query!(
		r#"
		INSERT INTO subscriptions (id, email, name, subscribed_at, status)
		VALUES ($1, $2, $3, $4, 'pending_confirmation')
		"#,
		subscriber_id,
		new_subscriber.email.as_ref(),
		new_subscriber.name.as_ref(),
		Utc::now()
	)
	.execute(transaction)
	.await
	.map_err(|e| {
		tracing::error!("Failed to execute query: {:?}", e);
		e
	})?;

	Ok(subscriber_id)
}

#[tracing::instrument(
name = "Send a confirmation email to a new subscriber",
skip(email_client, new_subscriber, base_url)
)]
pub async fn send_confirmation_email(
	email_client: &EmailClient,
	new_subscriber: NewSubscriber,
	base_url: &str,
	subscription_token: &str,
) -> Result<(), reqwest::Error> {
	let confirmation_link = format!(
		"{}/subscriptions/confirm?subscription_token={}",
		base_url,
		subscription_token,
	);
	let plain_body = &format!(
		"Welcome to our newsletter!\nVisit {} to confirm your subscription.",
		confirmation_link
	);
	let html_body = &format!(
		"Welcome to our newsletter!<br />/\
	 	 Click <a href=\"{}\">here</a> to confirm your subscription.",
		confirmation_link
	);
	let subject = "Welcome!";

	email_client.send_email(
		new_subscriber.email,
		subject,
		&html_body,
		&plain_body
	)
	.await
}

pub fn generate_subscriptions_token() -> String {
	let mut rng = thread_rng();
	std::iter::repeat_with(|| rng.sample(Alphanumeric))
		.map(char::from)
		.take(25)
		.collect()
}

#[tracing::instrument(
name = "Store subscription token in the database",
skip(subscription_token, transaction)
)]
pub async fn store_token(
	subscriber_id: Uuid,
	subscription_token: &str,
	transaction: &mut Transaction<'_, Postgres>,
) -> Result<(), StoreTokenError> {
	sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id)
        VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    )
		.execute(transaction)
		.await
		.map_err(|e| {
			tracing::error!("Failed to execute query: {:?}", e);
			StoreTokenError(e)
		})?;
	Ok(())
}
		
