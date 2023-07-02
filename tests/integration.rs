mod utils;

use rustgrok;

#[tokio::test]
async fn test_start_server() {
    let _server = tokio::spawn(async move {
        utils::start_server().await.unwrap();
    });
}
