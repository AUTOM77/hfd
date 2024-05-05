use tokio;

#[tokio::main]
async fn main() {
    let api = libhfd::api::tokio::Api::new().unwrap();

    let _filename = api
        .model("ByteDance/Hyper-SD".to_string())
        .get("Hyper-SDXL-8steps-lora.safetensors")
        .await
        .unwrap();
}