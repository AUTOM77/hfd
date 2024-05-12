use tokio;

#[tokio::main]
async fn main() {
    let start_time = std::time::Instant::now();

    let api = libhfd::api::tokio::Api::new().unwrap();

    let _filename = api
        .model("ByteDance/Hyper-SD".to_string())
        .get("Hyper-SDXL-8steps-lora.safetensors")
        .await
        .unwrap();
    println!("Processing time: {:?}", start_time.elapsed());
}