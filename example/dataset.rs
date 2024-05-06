use tokio;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    let start_time = std::time::Instant::now();

    let nums: Vec<Vec<u32>> = (0..204)
        .collect::<Vec<_>>()
        .chunks(10)
        .map(|chunk| chunk.to_vec())
        .collect();

    for chunk in nums{
        let mut tasks: Vec<_> = chunk.into_iter()
            .map(|i| tokio::spawn(async move {
                let api = libhfd::api::tokio::ApiBuilder::new()
                    .with_endpoint("https://hf-mirror.com")
                    .with_token("hf_xxxxxxxx")
                    .build()
                    .unwrap();
                let _filename = api
                    .dataset("OpenVideo/pexels-raw".to_string())
                    .get(format!("data/{:06}.parquet", i).as_ref())
                    .await;
            }))
            .collect();

        for task in tasks {
            task.await.unwrap();
        }
    }

    println!("Processing time: {:?}", start_time.elapsed());
}