use tokio;

#[tokio::main]
async fn main() {
    let _build = libhfd::api::tokio::ApiBuilder::new();
    let api = _build.with_endpoint("https://hf-mirror.com")
        .build()
        .unwrap();

    let _filename = api
        .model("ByteDance/Hyper-SD".to_string())
        .get("Hyper-SDXL-1step-Unet-Comfyui.fp16.safetensors")
        .await
        .unwrap();
}