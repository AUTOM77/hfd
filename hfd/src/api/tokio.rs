use super::RepoInfo;
use crate::{Cache, Repo, RepoType};
use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;
use reqwest::{
    header::{
        HeaderMap, HeaderName, HeaderValue, InvalidHeaderValue, ToStrError, AUTHORIZATION,
        CONTENT_RANGE, LOCATION, RANGE, USER_AGENT,
    },
    redirect::Policy,
    Client, Error as ReqwestError, RequestBuilder,
};
use std::num::ParseIntError;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::sync::{AcquireError, Semaphore, TryAcquireError};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, Error)]

pub enum ApiError {
    #[error("Header {0} is missing")]
    MissingHeader(HeaderName),

    #[error("Header {0} is invalid")]
    InvalidHeader(HeaderName),

    #[error("Invalid header value {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),

    #[error("header value is not a string")]
    ToStr(#[from] ToStrError),

    #[error("request error: {0}")]
    RequestError(#[from] ReqwestError),

    #[error("Cannot parse int")]
    ParseIntError(#[from] ParseIntError),

    #[error("I/O error {0}")]
    IoError(#[from] std::io::Error),

    #[error("Too many retries: {0}")]
    TooManyRetries(Box<ApiError>),

    #[error("Try acquire: {0}")]
    TryAcquireError(#[from] TryAcquireError),

    #[error("Acquire: {0}")]
    AcquireError(#[from] AcquireError),
}

#[derive(Debug)]
pub struct ApiBuilder {
    endpoint: String,
    cache: Cache,
    url_template: String,
    token: Option<String>,
    max_files: usize,
    chunk_size: usize,
    parallel_failures: usize,
    max_retries: usize,
    progress: bool,
}

impl Default for ApiBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiBuilder {
    pub fn new() -> Self {
        let cache = Cache::default();
        Self::from_cache(cache)
    }

    pub fn from_cache(cache: Cache) -> Self {
        let token = cache.token();

        let progress = true;

        Self {
            endpoint: "https://huggingface.co".to_string(),
            url_template: "{endpoint}/{repo_id}/resolve/{revision}/{filename}".to_string(),
            cache,
            token,
            max_files: num_cpus::get(),
            chunk_size: 10_000_000,
            parallel_failures: 0,
            max_retries: 0,
            progress,
        }
    }

    pub fn with_progress(mut self, progress: bool) -> Self {
        self.progress = progress;
        self
    }

    pub fn with_cache_dir(mut self, cache_dir: PathBuf) -> Self {
        self.cache = Cache::new(cache_dir);
        self
    }

    pub fn with_token(mut self, token: Option<String>) -> Self {
        self.token = token;
        self
    }

    fn build_headers(&self) -> Result<HeaderMap, ApiError> {
        let mut headers = HeaderMap::new();
        let user_agent = format!("unkown/None; {NAME}/{VERSION}; rust/unknown");
        headers.insert(USER_AGENT, HeaderValue::from_str(&user_agent)?);
        if let Some(token) = &self.token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))?,
            );
        }
        Ok(headers)
    }

    pub fn build(self) -> Result<Api, ApiError> {
        let headers = self.build_headers()?;
        let client = Client::builder().default_headers(headers.clone()).build()?;

        let relative_redirect_policy = Policy::custom(|attempt| {
            if attempt.previous().len() > 10 {
                return attempt.error("too many redirects");
            }

            if let Some(last) = attempt.previous().last() {
                if last.make_relative(attempt.url()).is_none() {
                    return attempt.stop();
                }
            }

            attempt.follow()
        });

        let relative_redirect_client = Client::builder()
            .redirect(relative_redirect_policy)
            .default_headers(headers)
            .build()?;
        Ok(Api {
            endpoint: self.endpoint,
            url_template: self.url_template,
            cache: self.cache,
            client,
            relative_redirect_client,
            max_files: self.max_files,
            chunk_size: self.chunk_size,
            parallel_failures: self.parallel_failures,
            max_retries: self.max_retries,
            progress: self.progress,
        })
    }
}

#[derive(Debug)]
struct Metadata {
    commit_hash: String,
    etag: String,
    size: usize,
}

#[derive(Clone, Debug)]
pub struct Api {
    endpoint: String,
    url_template: String,
    cache: Cache,
    client: Client,
    relative_redirect_client: Client,
    max_files: usize,
    chunk_size: usize,
    parallel_failures: usize,
    max_retries: usize,
    progress: bool,
}

fn make_relative(src: &Path, dst: &Path) -> PathBuf {
    let path = src;
    let base = dst;

    assert_eq!(
        path.is_absolute(),
        base.is_absolute(),
        "This function is made to look at absolute paths only"
    );
    let mut ita = path.components();
    let mut itb = base.components();

    loop {
        match (ita.next(), itb.next()) {
            (Some(a), Some(b)) if a == b => (),
            (some_a, _) => {
                let mut new_path = PathBuf::new();
                for _ in itb {
                    new_path.push(Component::ParentDir);
                }
                if let Some(a) = some_a {
                    new_path.push(a);
                    for comp in ita {
                        new_path.push(comp);
                    }
                }
                return new_path;
            }
        }
    }
}

fn symlink_or_rename(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    if dst.exists() {
        return Ok(());
    }

    let rel_src = make_relative(src, dst);
    #[cfg(target_os = "windows")]
    {
        if std::os::windows::fs::symlink_file(rel_src, dst).is_err() {
            std::fs::rename(src, dst)?;
        }
    }

    #[cfg(target_family = "unix")]
    std::os::unix::fs::symlink(rel_src, dst)?;

    Ok(())
}

fn jitter() -> usize {
    rand::thread_rng().gen_range(0..=500)
}

fn exponential_backoff(base_wait_time: usize, n: usize, max: usize) -> usize {
    (base_wait_time + n.pow(2) + jitter()).min(max)
}

impl Api {
    pub fn new() -> Result<Self, ApiError> {
        ApiBuilder::new().build()
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    async fn metadata(&self, url: &str) -> Result<Metadata, ApiError> {
        let response = self
            .relative_redirect_client
            .get(url)
            .header(RANGE, "bytes=0-0")
            .send()
            .await?;
        let response = response.error_for_status()?;
        let headers = response.headers();
        let header_commit = HeaderName::from_static("x-repo-commit");
        let header_linked_etag = HeaderName::from_static("x-linked-etag");
        let header_etag = HeaderName::from_static("etag");

        let etag = match headers.get(&header_linked_etag) {
            Some(etag) => etag,
            None => headers
                .get(&header_etag)
                .ok_or(ApiError::MissingHeader(header_etag))?,
        };

        let etag = etag.to_str()?.to_string().replace('"', "");
        let commit_hash = headers
            .get(&header_commit)
            .ok_or(ApiError::MissingHeader(header_commit))?
            .to_str()?
            .to_string();

        let response = if response.status().is_redirection() {
            self.client
                .get(headers.get(LOCATION).unwrap().to_str()?.to_string())
                .header(RANGE, "bytes=0-0")
                .send()
                .await?
        } else {
            response
        };
        let headers = response.headers();
        let content_range = headers
            .get(CONTENT_RANGE)
            .ok_or(ApiError::MissingHeader(CONTENT_RANGE))?
            .to_str()?;

        let size = content_range
            .split('/')
            .last()
            .ok_or(ApiError::InvalidHeader(CONTENT_RANGE))?
            .parse()?;
        Ok(Metadata {
            commit_hash,
            etag,
            size,
        })
    }

    pub fn repo(&self, repo: Repo) -> ApiRepo {
        ApiRepo::new(self.clone(), repo)
    }

    pub fn model(&self, model_id: String) -> ApiRepo {
        self.repo(Repo::new(model_id, RepoType::Model))
    }

    pub fn dataset(&self, model_id: String) -> ApiRepo {
        self.repo(Repo::new(model_id, RepoType::Dataset))
    }

    pub fn space(&self, model_id: String) -> ApiRepo {
        self.repo(Repo::new(model_id, RepoType::Space))
    }
}

#[derive(Debug)]
pub struct ApiRepo {
    api: Api,
    repo: Repo,
}

impl ApiRepo {
    fn new(api: Api, repo: Repo) -> Self {
        Self { api, repo }
    }
}

impl ApiRepo {
    pub fn url(&self, filename: &str) -> String {
        let endpoint = &self.api.endpoint;
        let revision = &self.repo.url_revision();
        self.api
            .url_template
            .replace("{endpoint}", endpoint)
            .replace("{repo_id}", &self.repo.url())
            .replace("{revision}", revision)
            .replace("{filename}", filename)
    }

    async fn download_tempfile(
        &self,
        url: &str,
        length: usize,
        progressbar: Option<ProgressBar>,
    ) -> Result<PathBuf, ApiError> {
        let mut handles = vec![];
        let semaphore = Arc::new(Semaphore::new(self.api.max_files));
        let parallel_failures_semaphore = Arc::new(Semaphore::new(self.api.parallel_failures));
        let filename = self.api.cache.temp_path();

        tokio::fs::File::create(&filename)
            .await?
            .set_len(length as u64)
            .await?;

        let chunk_size = self.api.chunk_size;
        for start in (0..length).step_by(chunk_size) {
            let url = url.to_string();
            let filename = filename.clone();
            let client = self.api.client.clone();

            let stop = std::cmp::min(start + chunk_size - 1, length);
            let permit = semaphore.clone().acquire_owned().await?;
            let parallel_failures = self.api.parallel_failures;
            let max_retries = self.api.max_retries;
            let parallel_failures_semaphore = parallel_failures_semaphore.clone();
            let progress = progressbar.clone();
            handles.push(tokio::spawn(async move {
                let mut chunk = Self::download_chunk(&client, &url, &filename, start, stop).await;
                let mut i = 0;
                if parallel_failures > 0 {
                    while let Err(dlerr) = chunk {
                        let parallel_failure_permit =
                            parallel_failures_semaphore.clone().try_acquire_owned()?;

                        let wait_time = exponential_backoff(300, i, 10_000);
                        tokio::time::sleep(tokio::time::Duration::from_millis(wait_time as u64))
                            .await;

                        chunk = Self::download_chunk(&client, &url, &filename, start, stop).await;
                        i += 1;
                        if i > max_retries {
                            return Err(ApiError::TooManyRetries(dlerr.into()));
                        }
                        drop(parallel_failure_permit);
                    }
                }
                drop(permit);
                if let Some(p) = progress {
                    p.inc((stop - start) as u64);
                }
                chunk
            }));
        }

        let results: Vec<Result<Result<(), ApiError>, tokio::task::JoinError>> =
            futures::future::join_all(handles).await;
        let results: Result<(), ApiError> = results.into_iter().flatten().collect();
        results?;
        if let Some(p) = progressbar {
            p.finish();
        }
        Ok(filename)
    }

    async fn download_chunk(
        client: &reqwest::Client,
        url: &str,
        filename: &PathBuf,
        start: usize,
        stop: usize,
    ) -> Result<(), ApiError> {
        let range = format!("bytes={start}-{stop}");
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(filename)
            .await?;
        file.seek(SeekFrom::Start(start as u64)).await?;
        let response = client
            .get(url)
            .header(RANGE, range)
            .send()
            .await?
            .error_for_status()?;
        let content = response.bytes().await?;
        file.write_all(&content).await?;
        Ok(())
    }

    pub async fn get(&self, filename: &str) -> Result<PathBuf, ApiError> {
        if let Some(path) = self.api.cache.repo(self.repo.clone()).get(filename) {
            Ok(path)
        } else {
            self.download(filename).await
        }
    }

    pub async fn download(&self, filename: &str) -> Result<PathBuf, ApiError> {
        let url = self.url(filename);
        let metadata = self.api.metadata(&url).await?;
        let cache = self.api.cache.repo(self.repo.clone());

        let blob_path = cache.blob_path(&metadata.etag);
        std::fs::create_dir_all(blob_path.parent().unwrap())?;

        let progressbar = if self.api.progress {
            let progress = ProgressBar::new(metadata.size as u64);
            progress.set_style(
                ProgressStyle::with_template(
                    "{msg} [{elapsed_precise}] [{wide_bar}] {bytes}/{total_bytes} {bytes_per_sec} ({eta})",
                )
                    .unwrap(),
            );
            let maxlength = 30;
            let message = if filename.len() > maxlength {
                format!("..{}", &filename[filename.len() - maxlength..])
            } else {
                filename.to_string()
            };
            progress.set_message(message);
            Some(progress)
        } else {
            None
        };

        let tmp_filename = self
            .download_tempfile(&url, metadata.size, progressbar)
            .await?;

        tokio::fs::rename(&tmp_filename, &blob_path).await?;

        let mut pointer_path = cache.pointer_path(&metadata.commit_hash);
        pointer_path.push(filename);
        std::fs::create_dir_all(pointer_path.parent().unwrap()).ok();

        symlink_or_rename(&blob_path, &pointer_path)?;
        cache.create_ref(&metadata.commit_hash)?;

        Ok(pointer_path)
    }

    pub async fn info(&self) -> Result<RepoInfo, ApiError> {
        Ok(self.info_request().send().await?.json().await?)
    }

    pub fn info_request(&self) -> RequestBuilder {
        let url = format!("{}/api/{}", self.api.endpoint, self.repo.api_url());
        self.api.client.get(url)
    }
}

