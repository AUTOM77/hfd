use tokio;

#[tokio::main]
async fn main() {
    let _build = libhfd::api::tokio::ApiBuilder::new();

    let api = _build.with_endpoint("https://hf-mirror.com")
        .with_token("xxxxxxxxxxx")
        .build()
        .unwrap();

    let _filename = api
        .dataset("OpenVideo/pexels-raw".to_string())
        .get(format!("data/{:06}.parquet", 0).as_ref())
        .await;
}