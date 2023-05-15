use sn_test::db;
use tiberius::Query;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = db::MssqlConnParams::with_host_and_db("hiiwinbl5", "SNDBaseDev")
        .set_auth("SNUser", "BestNest1445");

    let mut cnxn = db::connect(db::DbConfig::Sigmanest(cfg)).await;

    let select = Query::new("select * from Stock");
    for row in select.query(&mut cnxn).await?.into_first_result().await? {
        println!("{:?}", row);
    }

    Ok(())
}
