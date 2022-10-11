use std::env;

use anyhow::Result;
use futures::future::join_all;
use jd_com::account::get_accounts;
use jd_farm::JClient;
use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    pretty_env_logger::init();

    let jd_cookie = env::var("JD_COOKIE");

    if jd_cookie.is_err() {
        info!("未找到环境变量JD_COOKIE.");
        return Ok(());
    }

    let accounts = get_accounts(jd_cookie.unwrap());

    let mut handles = Vec::new();

    for account in accounts {
        let handle = tokio::spawn(async move {
            let client = JClient::new(account);
            let _ = client.run().await;
        });
        handles.push(handle);
    }

    join_all(handles).await;

    Ok(())
}
