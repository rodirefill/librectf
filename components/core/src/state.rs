use std::sync::{Arc, Mutex};

use failure::Error;
use tera::{Context, Tera};

use config::{Config, FilestoreConfig, WebConfig};
use db::{establish_connection, Connection, Pool};

struct InnerState {
    pub(super) db_pool: Pool,
}

#[derive(Clone)]
pub struct State {
    inner: Arc<InnerState>,
    config: Arc<Config>,
    tera: Arc<Mutex<Tera>>,
}

impl State {
    pub fn from(config: &Config) -> State {
        let db_pool = establish_connection(&config.database_url);

        let inner = Arc::new(InnerState { db_pool });
        let config = Arc::new(config.clone());
        let tera = Arc::new(Mutex::new(Tera::default()));

        State {
            inner,
            config,
            tera,
        }
    }

    pub fn get_web_config(&self) -> Option<&WebConfig> {
        self.config.web.as_ref()
    }

    pub fn get_filestore_config(&self) -> Option<&FilestoreConfig> {
        self.get_web_config().and_then(|cfg| cfg.filestore.as_ref())
    }

    pub fn render(&self, page: impl AsRef<str>, ctx: &Context) -> Result<String, Error> {
        let t = self
            .tera
            .lock()
            .map_err(|err| format_err!("Internal error acquiring Tera lock: {}", err))?;
        t.render(page.as_ref(), ctx)
            .map_err(|err| format_err!("Internal error rendering template: {}", err))
    }

    pub fn add_templates(&mut self, templates: Vec<(&str, &str)>) -> Result<(), Error> {
        let mut t = self
            .tera
            .lock()
            .map_err(|err| format_err!("Internal error acquiring Tera lock: {}", err))?;
        t.add_raw_templates(templates)
            .map_err(|err| format_err!("Error adding Tera templates: {}", err))
    }

    pub fn get_connection(&self) -> Result<Connection, Error> {
        match self.inner.db_pool.get() {
            Ok(conn) => Ok(Connection(conn)),
            Err(err) => Err(format_err!("Database connection error: {}", err)),
        }
    }
}
