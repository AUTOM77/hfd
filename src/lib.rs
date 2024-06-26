use std::str::FromStr;
use std::path::PathBuf;
use reqwest::header::{HeaderMap, AUTHORIZATION, CONTENT_RANGE, RANGE, USER_AGENT};
use tokio::time::Duration;
use tokio::io::{AsyncSeekExt, SeekFrom};
use tokio_stream::StreamExt;

const CHUNK_SIZE: usize = 100_000_000;
const CHUNK_SIZE_XL: usize = 10_000_000_000;

#[derive(Debug)]
pub struct HfURL {
    endpoint: String,
    repo_type: Option<String>,
    repo_id: String,
}

impl HfURL {
    pub fn new(endpoint: String, repo_type: Option<String>, repo_id: String) -> Self {
        Self { endpoint, repo_type, repo_id }
    }

    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = endpoint.to_string();
        self
    }

    pub fn api(&self) -> String {
        let repo_path = match &self.repo_type {
            Some(repo_type) => repo_type.clone(),
            _ => "models".to_string(),
        };
        format!("https://{}/api/{}/{}", self.endpoint, repo_path, self.repo_id)
    }

    pub fn path(&self, fname: &str) -> String {
        let repo_path = match &self.repo_type {
            Some(repo_type) => format!("{}/{}", repo_type, self.repo_id),
            _ => self.repo_id.clone(),
        };
        format!("https://{}/{}/resolve/main/{}", self.endpoint, repo_path, fname)
    }
}

impl FromStr for HfURL {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('/').skip(2);
        let endpoint = match parts.next() {
            Some(ep) => ep.to_string(),
            None => return Err("Missing endpoint"),
        };

        let mut repo_type = None;

        if let Some(next_part) = parts.clone().next() {
            repo_type = match next_part {
                "datasets" | "spaces" => Some(next_part.to_string()),
                _ => None,
            };

            if repo_type.is_some() {
                parts.next();
            }
        }

        let owner = parts.next().ok_or("Missing owner")?;
        let repo = parts.next().ok_or("Missing repo")?;
        let repo_id = format!("{}/{}", owner, repo);

        Ok(HfURL::new(endpoint, repo_type, repo_id))
    }
}

#[derive(Debug)]
pub struct HfClient {
    headers: HeaderMap,
    hf_url: HfURL,

    root: PathBuf,
}

async fn download_chunk(
        headers: HeaderMap,
        url: String,
        path: PathBuf, 
        s: usize, 
        e: usize
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client= reqwest::Client::builder()
        .default_headers(headers.clone())
        .pool_max_idle_per_host(0)
        .build()?;

    let range = format!("bytes={s}-{e}");

    let response = client.get(&url).header(RANGE, range).send().await?;
    let mut stream = response.bytes_stream();

    let mut file = tokio::fs::OpenOptions::new().write(true).open(path).await?;
    file.seek(SeekFrom::Start(s as u64)).await?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        tokio::io::copy(&mut chunk.as_ref(), &mut file).await?;
    }
    Ok(())
}

async fn download_chunk_with_retry(
    headers: HeaderMap,
    url: String,
    path: PathBuf,
    s: usize,
    e: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    const MAX_RETRIES: usize = 100;
    let mut retries = 0;

    let mut error: Option<Box<dyn std::error::Error + Send + Sync>> = None;

    while retries < MAX_RETRIES {
        match download_chunk(headers.clone(), url.clone(), path.clone(), s, e).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                println!("Retry {:#?} with {:?} times", e, retries);
                error = Some(e);
                retries += 1;
                std::thread::sleep(Duration::from_secs(10));
            }
        }
    }

    Err(error.unwrap_or_else(|| Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Exhausted all retries",
    ))))
}

async fn download(
        headers: HeaderMap,
        url: String,
        path: PathBuf, 
        chunk_size: usize
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .default_headers(headers.clone())
        .pool_max_idle_per_host(0)
        .build()?;

    let response = client.get(&url).header(RANGE, "bytes=0-0").send().await?;
    let length: usize = response
        .headers()
        .get(CONTENT_RANGE)
        .ok_or("Content-Length not found")?
        .to_str()?.rsplit('/').next()
        .and_then(|s| s.parse().ok())
        .ok_or("Failed to parse size")?;
    
    tokio::fs::File::create(&path)
        .await?
        .set_len(length as u64)
        .await?;

    let mut tasks = Vec::new();

    for s in (0..length).step_by(chunk_size) {
        let e = std::cmp::min(s + chunk_size - 1, length);
        tasks.push(
            tokio::spawn(download_chunk_with_retry(headers.clone(), url.clone(), path.clone(), s, e))
        );
    }

    for task in tasks {
        task.await;
    }

    Ok(())
}

impl HfClient {
    pub fn new(headers: HeaderMap, hf_url: HfURL) -> Self {
        let default = match std::env::var("HF_HOME") {
            Ok(home) => home,
            Err(_) => ".".to_string()
        };

        let root = PathBuf::from(default).join(hf_url.repo_id.clone());
        Self { headers, hf_url, root }
    }

    pub fn build(url: &str) -> Result<Self, Box<dyn std::error::Error>>{
        let hf_url = url.parse()?;
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, "hyper-client-http2".parse()?);
        Ok(Self::new(headers, hf_url))
    }

    pub fn apply_token(mut self, _token: Option<&str>) -> Self{
        if let Some(token) = _token {
            self.headers.insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        }
        self
    }

    pub fn apply_root(mut self, _root: Option<&str>) -> Self{
        if let Some(root) = _root {
            self.root = PathBuf::from(root).join(self.hf_url.repo_id.clone());
        }
        self
    }

    pub fn apply_endpoint(mut self, _endpoint: Option<&str>) -> Self{
        if let Some(ep) = _endpoint {
            self.hf_url = self.hf_url.with_endpoint(ep);
        }
        self
    }

    async fn list_files(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()?;
        let api = self.hf_url.api();
        let response = client.get(api)
            .headers(self.headers.clone())
            .send().await?
            .json::<serde_json::Value>()
            .await?;

        let mut files: Vec<String> = Vec::new();

        if let Some(siblings) = response["siblings"].as_array() {
            let mut _files: Vec<String> = siblings.into_iter()
                .map(|f|f.get("rfilename").expect("filename").as_str())
                .flatten()
                .map(|x| x.into())
                .collect();
            files.append(&mut _files);
        }
        Ok(files)
    }

    fn create_dir_all(&self, files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        for file in files {
            if let Some(parent) = self.root.join(file).parent() {
                let _ = std::fs::create_dir_all(parent)?;
            }
        }
        Ok(())
    }

    pub async fn download_all(&self, limit: Option<usize>) -> Result<(), Box<dyn std::error::Error>> {
        let files_all = self.list_files().await?;
        let total_file = files_all.len();

        let num = limit.unwrap_or(total_file);
        let files: Vec<_> = files_all.into_iter().take(num).collect();

        let _ = self.create_dir_all(files.clone());

        for chunks in files.chunks(5){
            let mut tasks = Vec::new();
            for file in chunks{
                let url = self.hf_url.path(&file);
                let path = self.root.join(&file);
                let headers = self.headers.clone();

                if self.hf_url.endpoint.contains("face"){
                    tasks.push(
                        download(headers, url, path, CHUNK_SIZE)
                    );
                }
                else {
                    tasks.push(
                        download(headers, url, path, CHUNK_SIZE_XL)
                    );
                }
            }

            for task in tasks {
                task.await;
            }
        }
        Ok(())
    }
}

pub fn interface(
    repo: &str, 
    token: Option<&str>,
    target: Option<&str>,
    mirror: Option<&str>,
    limit: Option<usize>,
){
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let client = HfClient::build(repo)
        .unwrap()
        .apply_token(token)
        .apply_root(target)
        .apply_endpoint(mirror);

    let _ = rt.block_on(
        client.download_all(limit)
    );
}