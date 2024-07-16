use crate::celestia::l2_router::{
    types::{L2Config, L2Type},
    L2Router,
};
use std::collections::HashMap;

use ethers::core::rand::rngs::mock;
use serde_json::json;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn test_optimism_router() {
    let l2_router = create_test_router().await;

    let commitment =
        hex::decode("f1a51990b5a358a2376e85648b489138ca38533e2b86e0283d41ceeebcf058ea").unwrap();
    let batch_metadata = l2_router
        .get_l2_batch_metadata(
            760960,
            "0x00000000000000000000000000000000000000000008e5f679bf7116cb",
            &commitment,
        )
        .await
        .unwrap();

    assert_eq!(batch_metadata.chain_type, L2Type::Optimism);
    assert_eq!(batch_metadata.chain_id, 123420111);
    assert_eq!(batch_metadata.l2_batch_id, "5");
    assert_eq!(batch_metadata.l2_start_block, 29996);
    assert_eq!(batch_metadata.l2_end_block, 33082);
    assert_eq!(batch_metadata.l2_batch_tx_count, 1);
    assert_eq!(
        batch_metadata.l2_blockscout_url,
        "http://raspberry.blockscout.com",
    );
    assert_eq!(
        batch_metadata.l1_tx_hash,
        "0xf41211e966ec23032dde713d1f775ae5cb07dc5e15951281e6844d74cc02a930",
    );
    assert_eq!(batch_metadata.l1_tx_timestamp, 1703067444);
    assert_eq!(batch_metadata.related_blobs.len(), 1);
}

#[tokio::test]
#[should_panic(expected = "unsupported chain type: Arbitrum")]
async fn test_arbitrum_router() {
    let l2_router = create_test_router().await;
    println!("{}", toml::to_string(&l2_router).unwrap());
    let commitment =
        hex::decode("f1a51990b5a358a2376e85648b489138ca38533e2b86e0283d41ceeebcf058ea").unwrap();
    l2_router
        .get_l2_batch_metadata(
            760960,
            "0x00000000000000000000000000000000000000000008e5f679bf7116cf",
            &commitment,
        )
        .await
        .unwrap();
}

async fn create_test_router() -> L2Router {
    let mock_server = create_optimism_blockscout_mock().await;
    let mut routes: HashMap<String, L2Config> = HashMap::new();
    routes.insert(
        "0x00000000000000000000000000000000000000000008e5f679bf7116cb".to_string(),
        L2Config {
            chain_type: L2Type::Optimism,
            chain_id: 123420111,
            l2_api_url: mock_server.uri(),
            l2_blockscout_url: "http://raspberry.blockscout.com".to_string(),
        },
    );
    routes.insert(
        "0x00000000000000000000000000000000000000000008e5f679bf7116cf".to_string(),
        L2Config {
            chain_type: L2Type::Arbitrum,
            chain_id: 123,
            l2_api_url: String::new(),
            l2_blockscout_url: String::new(),
        },
    );

    L2Router::new(routes).unwrap()
}

async fn create_optimism_blockscout_mock() -> MockServer {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(format!("api/v2/optimism/batches/da/celestia/760960/0xf1a51990b5a358a2376e85648b489138ca38533e2b86e0283d41ceeebcf058ea")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(
            {
                "batch_data_container": "in_celestia",
                "blobs": [
                  {
                    "commitment": "0xf1a51990b5a358a2376e85648b489138ca38533e2b86e0283d41ceeebcf058ea",
                    "height": 760960,
                    "l1_timestamp": "2023-12-20T10:17:12.000000Z",
                    "l1_transaction_hash": "0xf41211e966ec23032dde713d1f775ae5cb07dc5e15951281e6844d74cc02a930",
                    "namespace": "0x00000000000000000000000000000000000000000008e5f679bf7116cb"
                  },
                  {
                    "commitment": "0x3834d3a92ede97db07defc291c848b6085389c236f88c52b67a933271f316fee",
                    "height": 760961,
                    "l1_timestamp": "2023-12-20T10:17:24.000000Z",
                    "l1_transaction_hash": "0x9abc0df13890e8c0818b448b15056ecd96368dc2b4f625c1232285e05e5b3826",
                    "namespace": "0x00000000000000000000000000000000000000000008e5f679bf7116cb"
                  }
                ],
                "internal_id": 5,
                "l1_timestamp": "2023-12-20T10:17:24.000000Z",
                "l1_tx_hashes": [
                  "0xf41211e966ec23032dde713d1f775ae5cb07dc5e15951281e6844d74cc02a930",
                  "0x9abc0df13890e8c0818b448b15056ecd96368dc2b4f625c1232285e05e5b3826"
                ],
                "l2_block_start": 29996,
                "l2_block_end": 33082,
                "tx_count": 1
              }
        )))
        .mount(&mock_server)
        .await;
    mock_server
}
