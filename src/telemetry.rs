use tracing::subscriber::set_global_default;
use tracing::Subscriber;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{layer::SubscriberExt, fmt::MakeWriter, EnvFilter, Registry};
use tokio::task::JoinHandle;


pub fn get_subscriber<S>(
	name: String, 
	env_filter: String,
	sink: S
) -> impl Subscriber + Send + Sync 
where S: for<'a> MakeWriter<'a> + Send + Sync + 'static
{
	let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
	let formatting_layer = BunyanFormattingLayer::new(name, sink);
	Registry::default()
		.with(env_filter)
		.with(JsonStorageLayer)
		.with(formatting_layer)
}

pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
	LogTracer::init().expect("failed to set logger");
	set_global_default(subscriber).expect("failed to set subscriber");
}

pub fn spawn_blocking_with_tracing<F, R>(f: F) -> JoinHandle<R>
	where
		F: FnOnce() -> R + Send + 'static,
		R: Send + 'static,
{
	let current_span = tracing::Span::current();
	tokio::task::spawn_blocking(move || current_span.in_scope(f))
}
