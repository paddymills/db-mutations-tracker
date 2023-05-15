
use std::sync::RwLock;

use surrealdb::Surreal;
use surrealdb::engine::local::{Db, Mem};

static TRACKING_DB: Surreal<Db> = surrealdb::Surreal::init();
static DB_IS_CONNECTED: RwLock<bool> = RwLock::new(false);

/// Gets the tracking database.
/// 
/// first checks if the database is connected and connects if it is not
pub async fn get_db<'a>() -> surrealdb::Result<&'a Surreal<Db>> {
    // read only check, does not block others trying to do the same
    if !*DB_IS_CONNECTED.read().unwrap() {

        // blocking call to get exclusive access to init flag
        let mut is_connected = DB_IS_CONNECTED.write().unwrap();

        // make sure that another thread did make a connection since/during our initial read
        if !*is_connected {
            TRACKING_DB.connect::<Mem>(()).await?;
            TRACKING_DB.use_ns("mut_tracking").use_db("sigmanest").await?;
    
            *is_connected = true;
        }        
    }
    

    Ok(&TRACKING_DB)
}
