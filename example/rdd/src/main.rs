use tokio;
use reqwest::header::{RANGE, CONTENT_RANGE};
use tokio::io::{AsyncSeekExt, SeekFrom};
use tokio_stream::StreamExt;

const CHUNK_SIZE: usize = 10_000_000;
const URL: &str = "https://huggingface.co/ByteDance/Hyper-SD/resolve/main/Hyper-SDXL-1step-Unet-Comfyui.fp16.safetensors";
const FILE: &str = "fp16.safetensors";

async fn download_chunk(s: usize, e: usize) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .http2_keep_alive_timeout(tokio::time::Duration::from_secs(15)).build()?;
    let range = format!("bytes={s}-{e}");

    let response = client.get(URL).header(RANGE, range).send().await?;
    let mut stream = response.bytes_stream();

    let mut file = tokio::fs::OpenOptions::new().write(true).open(FILE).await?;
    file.seek(SeekFrom::Start(s as u64)).await?;    
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        tokio::io::copy(&mut chunk.as_ref(), &mut file).await?;
    }
    Ok(())
}

async fn download() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .http2_keep_alive_timeout(tokio::time::Duration::from_secs(15)).build()?;

    let response = client.get(URL).header(RANGE, "bytes=0-0").send().await?;
    let length: usize = response
        .headers()
        .get(CONTENT_RANGE)
        .ok_or("Content-Length not found")?
        .to_str()?.rsplit('/').next()
        .and_then(|s| s.parse().ok())
        .ok_or("Failed to parse size")?;
    
    let _ = tokio::fs::File::create(FILE).await?.set_len(length as u64).await?;

    let tasks: Vec<_> = (0..length)
        .into_iter()
        .step_by(CHUNK_SIZE)
        .map(|s| {
            let e = std::cmp::min(s + CHUNK_SIZE - 1, length);
            tokio::spawn(async move { download_chunk(s, e).await })
        })
        .collect();

    for task in tasks {
        let _ = task.await.unwrap();
    }
    Ok(())
}

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    let start_time = std::time::Instant::now();
    let _ = rt.block_on(download());
    println!("Processing time: {:?}", start_time.elapsed());
}
