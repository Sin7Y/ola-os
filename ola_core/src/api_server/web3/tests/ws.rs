use std::time::{Duration, Instant};

use jsonrpsee::{core::client::{Subscription, SubscriptionClientT}, rpc_params, ws_client::WsClientBuilder};
use ola_web3_decl::types::PubSubResult;

#[ignore]
#[tokio::test]
async fn test_subscriptions() {
    let client = WsClientBuilder::default()
            .build("ws://127.0.0.1:13003")
            .await.unwrap();
    let params = rpc_params!["block_proofs"];
    let mut subscription: Subscription<PubSubResult> = client
        .subscribe("ola_subscribe", params, "ola_unsubscribe")
        .await.unwrap();
    let start = Instant::now();
    let subscription_duration = Duration::from_secs(30);
    loop {
        if let Ok(resp) = tokio::time::timeout(subscription_duration, subscription.next()).await
        {
            match resp {
                Some(Ok(item)) => println!("receive new item: {:?}", item),
                None => panic!("OperationTimeout"),
                Some(Err(err)) => panic!("Error: {:?}", err),
            }
        } else {
            panic!("Receive Timeout")
        }
        if start.elapsed() > subscription_duration {
            break;
        }
    }
}