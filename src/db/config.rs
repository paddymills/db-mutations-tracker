use std::path::PathBuf;

use bb8_tiberius::IntoConfig;
use tiberius::{AuthMethod, Config};

#[derive(Debug)]
pub enum DbConfig {
    Sigmanest(MssqlConnParams),
    Temp(PathBuf),
}

/// Database connection
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct MssqlConnParams {
    /// Server name
    server: String,

    /// Server instance, if applicable
    instance: Option<String>,

    /// Database name (optional)
    database: Option<String>,

    /// User (optional)
    user: Option<String>,

    /// Password (if applicable)
    password: Option<String>,
}

impl MssqlConnParams {
    pub fn with_host_and_db(host: impl ToString, db: impl ToString) -> Self {
        Self {
            server: host.to_string(),
            database: Some(db.to_string()),

            ..Default::default()
        }
    }

    pub fn set_auth(mut self, user: impl ToString, pwd: impl ToString) -> Self {
        self.user = Some(user.to_string());
        self.password = Some(pwd.to_string());

        self
    }
}

impl IntoConfig for MssqlConnParams {
    fn into_config(self) -> tiberius::Result<tiberius::Config> {
        let mut config = Config::new();

        config.host(&self.server);
        config.database(&self.database.as_ref().unwrap());

        if let Some(inst) = &self.instance {
            config.instance_name(inst);
        }

        match &self.user {
            Some(username) => {
                config.authentication(AuthMethod::sql_server(username, &self.password.unwrap()));
            }
            None => {
                // use windows authentication
                config.authentication(AuthMethod::Integrated);
            }
        }

        config.trust_cert();

        Ok(config)
    }
}
