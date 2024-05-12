use std::str::FromStr;
use std::path::PathBuf;
use reqwest::header::{HeaderMap, AUTHORIZATION, CONTENT_RANGE, RANGE, USER_AGENT};
use tokio::time::Duration;
use tokio::io::{AsyncSeekExt, SeekFrom};
use tokio_stream::StreamExt;

const CHUNK_SIZE: usize = 10_000_000;

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
        headers: &HeaderMap,
        url: &str,
        path: &PathBuf, 
        s: usize, 
        e: usize
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client= reqwest::Client::builder()
            .default_headers(headers.clone())
            .pool_idle_timeout(Duration::from_millis(50))
            .pool_max_idle_per_host(2)
            .timeout(Duration::from_secs(30))
            .build()?;
        let range = format!("bytes={s}-{e}");

        let response = client.get(url).header(RANGE, range).send().await?;
        let mut stream = response.bytes_stream();

        let mut file = tokio::fs::OpenOptions::new().write(true).open(path).await?;
        file.seek(SeekFrom::Start(s as u64)).await?;    
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            tokio::io::copy(&mut chunk.as_ref(), &mut file).await?;
        }
    Ok(())
}

async fn download(
        headers: HeaderMap,
        url: String,
        path: PathBuf, 
        chunk_size: usize
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // let url = self.hf_url.path(&file);
        // let path = self.root.join(&file);
        let client = reqwest::Client::builder()
            .default_headers(headers.clone())
            .http2_keep_alive_timeout(Duration::from_secs(15)).build()?;

        let response = client.get(&url).header(RANGE, "bytes=0-0").send().await?;
        let length: usize = response
            .headers()
            .get(CONTENT_RANGE)
            .ok_or("Content-Length not found")?
            .to_str()?.rsplit('/').next()
            .and_then(|s| s.parse().ok())
            .ok_or("Failed to parse size")?;
        
        let _ = tokio::fs::File::create(&path).await?.set_len(length as u64).await?;

        let tasks: Vec<_> = (0..length)
            .into_iter()
            .step_by(chunk_size)
            .map(|s| {
                let _url = url.clone();
                let _path = path.clone();
                let headers = headers.clone();
                let e = std::cmp::min(s + chunk_size - 1, length);
                tokio::spawn(async move { download_chunk(&headers, &_url, &_path, s, e).await })
            })
            .collect();

        for task in tasks {
            let _ = task.await.unwrap();
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

    async fn list_files(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let client = reqwest::Client::builder()
            .http2_keep_alive_timeout(Duration::from_secs(15)).build()?;
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

    pub async fn download_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        let files = self.list_files().await?;
        let _ = self.create_dir_all(files.clone());
        let file_chunks: Vec<_> = files
            .chunks(30)
            .map(|chunk| chunk.to_owned())
            .collect();

        for fc in file_chunks{
            let tasks: Vec<_> = fc.into_iter()
                .map(|f| {
                    let url = self.hf_url.path(&f);
                    let path = self.root.join(&f);
                    let headers = self.headers.clone();
                    tokio::spawn(async move {let _ = download(headers, url, path, CHUNK_SIZE).await; })
                })
                .collect();

            for task in tasks {
                task.await.unwrap();
            }
        }
        Ok(())
    }
}

pub fn _rt(_url: &str, _token: Option<&str>, _dir: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let hfc = HfClient::build(_url)?
        .apply_token(_token)
        .apply_root(_dir);

    let _ = rt.block_on(hfc.download_all());
    Ok(())
}