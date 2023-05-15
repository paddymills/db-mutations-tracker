mod config;
use chrono::{NaiveDateTime, Utc};
pub use config::*;

use bb8::Pool;
use bb8_tiberius::{ConnectionManager, IntoConfig};
use tiberius::Client;
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

pub type DbPool = Pool<ConnectionManager>;
pub type DbClient = Client<Compat<TcpStream>>;

pub async fn connect(cfg: config::DbConfig) -> DbClient {
    use DbConfig::*;
    match cfg {
        Sigmanest(mssql) => {
            let mssql = mssql.into_config().expect("Failed to convert config");

            let tcp = TcpStream::connect(mssql.get_addr())
                .await
                .expect("failed to establish TcpStream");
            tcp.set_nodelay(true)
                .expect("Failed to set no delay for TcpStream");

            Client::connect(mssql, tcp.compat_write())
                .await
                .expect("Failed to connect db client")
        }
        Temp(_path) => todo!("impl SQLite"),
    }
}

pub async fn build_db_pool(config: impl IntoConfig, size: u32) -> DbPool {
    Pool::builder()
        .max_size(size)
        .build(
            ConnectionManager::build(config)
                .expect("ConnectionManager failed to connect to database"),
        )
        .await
        .expect("Bom Pool failed to build")
}

pub fn current_time() -> NaiveDateTime {
    Utc::now().naive_utc()
}
