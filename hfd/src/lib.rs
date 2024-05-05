use rand::{distributions::Alphanumeric, Rng};
use std::io::Write;
use std::path::PathBuf;

pub mod api;

#[derive(Debug, Clone, Copy)]
pub enum RepoType {
    Model,
    Dataset,
    Space,
}

#[derive(Clone, Debug)]
pub struct Cache {
    path: PathBuf,
}

impl Cache {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn token_path(&self) -> PathBuf {
        let mut path = self.path.clone();
        path.pop();
        path.push("token");
        path
    }

    pub fn token(&self) -> Option<String> {
        let token_filename = self.token_path();
        if !token_filename.exists() {
            log::info!("Token file not found {token_filename:?}");
        }
        match std::fs::read_to_string(token_filename) {
            Ok(token_content) => {
                let token_content = token_content.trim();
                if token_content.is_empty() {
                    None
                } else {
                    Some(token_content.to_string())
                }
            }
            Err(_) => None,
        }
    }

    pub fn repo(&self, repo: Repo) -> CacheRepo {
        CacheRepo::new(self.clone(), repo)
    }

    pub fn model(&self, model_id: String) -> CacheRepo {
        self.repo(Repo::new(model_id, RepoType::Model))
    }

    pub fn dataset(&self, model_id: String) -> CacheRepo {
        self.repo(Repo::new(model_id, RepoType::Dataset))
    }

    pub fn space(&self, model_id: String) -> CacheRepo {
        self.repo(Repo::new(model_id, RepoType::Space))
    }

    pub(crate) fn temp_path(&self) -> PathBuf {
        let mut path = self.path().clone();
        path.push("tmp");
        std::fs::create_dir_all(&path).ok();

        let s: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();
        path.push(s);
        path.to_path_buf()
    }
}

#[derive(Debug)]
pub struct CacheRepo {
    cache: Cache,
    repo: Repo,
}

impl CacheRepo {
    fn new(cache: Cache, repo: Repo) -> Self {
        Self { cache, repo }
    }

    pub fn get(&self, filename: &str) -> Option<PathBuf> {
        let commit_path = self.ref_path();
        let commit_hash = std::fs::read_to_string(commit_path).ok()?;
        let mut pointer_path = self.pointer_path(&commit_hash);
        pointer_path.push(filename);
        if pointer_path.exists() {
            Some(pointer_path)
        } else {
            None
        }
    }

    fn path(&self) -> PathBuf {
        let mut ref_path = self.cache.path.clone();
        ref_path.push(self.repo.folder_name());
        ref_path
    }

    fn ref_path(&self) -> PathBuf {
        let mut ref_path = self.path();
        ref_path.push("refs");
        ref_path.push(self.repo.revision());
        ref_path
    }

    pub fn create_ref(&self, commit_hash: &str) -> Result<(), std::io::Error> {
        let ref_path = self.ref_path();

        std::fs::create_dir_all(ref_path.parent().unwrap())?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&ref_path)?;
        file.write_all(commit_hash.trim().as_bytes())?;
        Ok(())
    }

    pub(crate) fn blob_path(&self, etag: &str) -> PathBuf {
        let mut blob_path = self.path();
        blob_path.push("blobs");
        blob_path.push(etag);
        blob_path
    }

    pub(crate) fn pointer_path(&self, commit_hash: &str) -> PathBuf {
        let mut pointer_path = self.path();
        pointer_path.push("snapshots");
        pointer_path.push(commit_hash);
        pointer_path
    }
}

impl Default for Cache {
    fn default() -> Self {
        let mut path = match std::env::var("HF_HOME") {
            Ok(home) => home.into(),
            Err(_) => {
                let mut cache = dirs::home_dir().expect("Cache directory cannot be found");
                cache.push(".cache");
                cache.push("huggingface");
                cache
            }
        };
        path.push("hub");
        Self::new(path)
    }
}

#[derive(Clone, Debug)]
pub struct Repo {
    repo_id: String,
    repo_type: RepoType,
    revision: String,
}

impl Repo {
    pub fn new(repo_id: String, repo_type: RepoType) -> Self {
        Self::with_revision(repo_id, repo_type, "main".to_string())
    }

    pub fn with_revision(repo_id: String, repo_type: RepoType, revision: String) -> Self {
        Self {
            repo_id,
            repo_type,
            revision,
        }
    }

    pub fn model(repo_id: String) -> Self {
        Self::new(repo_id, RepoType::Model)
    }

    pub fn dataset(repo_id: String) -> Self {
        Self::new(repo_id, RepoType::Dataset)
    }

    pub fn space(repo_id: String) -> Self {
        Self::new(repo_id, RepoType::Space)
    }

    pub fn folder_name(&self) -> String {
        let prefix = match self.repo_type {
            RepoType::Model => "models",
            RepoType::Dataset => "datasets",
            RepoType::Space => "spaces",
        };
        format!("{prefix}--{}", self.repo_id).replace('/', "--")
    }

    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub fn url(&self) -> String {
        match self.repo_type {
            RepoType::Model => self.repo_id.to_string(),
            RepoType::Dataset => {
                format!("datasets/{}", self.repo_id)
            }
            RepoType::Space => {
                format!("spaces/{}", self.repo_id)
            }
        }
    }

    pub fn url_revision(&self) -> String {
        self.revision.replace('/', "%2F")
    }

    pub fn api_url(&self) -> String {
        let prefix = match self.repo_type {
            RepoType::Model => "models",
            RepoType::Dataset => "datasets",
            RepoType::Space => "spaces",
        };
        format!("{prefix}/{}/revision/{}", self.repo_id, self.url_revision())
    }
}
