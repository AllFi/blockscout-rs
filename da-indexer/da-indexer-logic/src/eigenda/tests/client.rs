use crate::eigenda::client::Client;

#[tokio::test]
#[ignore]
pub async fn parse_eds_with_blobs() {
    let client = Client::new(
        "https://disperser-holesky.eigenda.xyz:443".to_string(),
        vec![5, 10, 15],
    );
    let blobs = client
        .retrieve_blobs(
            hex::decode("261C70F17FDF3A491FA8FFBF712718726153816E5284F273C364EAED2B53323C")
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(blobs.len() == 639)
}
