use serde::Deserialize;

pub mod tokio;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Siblings {
    pub rfilename: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RepoInfo {
    pub siblings: Vec<Siblings>,

    pub sha: String,
}