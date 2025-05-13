use reqwest::Client;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("URL parsing error: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("HTTP request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Path error: {0}")]
    PathError(String),
}

/// Path to a cache file.
#[derive(Debug)]
struct CachePath {
    dir: PathBuf,
    file: String,
}

impl CachePath {
    /// Create a new `CachePath` instance without validation.
    fn new(dir: PathBuf, file: String) -> Self {
        CachePath { dir, file }
    }
    /// Create a new `CachePath` instance, validating that `dir` does not contain
    /// a file extension and that `file` does.
    fn try_new(dir: PathBuf, file: String) -> Result<Self, FetchError> {
        if dir.extension().is_some() {
            return Err(FetchError::PathError(format!(
                "Directory name must not have an extension. Directory: {}",
                dir.display()
            )));
        }
        if Path::new(&file).extension().is_none() {
            return Err(FetchError::PathError(format!(
                "File name must have an extension but got: {}",
                file
            )));
        }
        Ok(CachePath { dir, file })
    }

    /// Create child directory of `base_dir` from a URL's path.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to parse and create the path file from
    /// * `base_dir` - The base directory to use as the parent folder
    ///
    /// # Returns
    ///
    /// * `Ok(CachePath)` - The cache path
    /// * `Err(FetchError)` - An error if the URL contains no path elements or if the last path
    ///     element does not seem to represent a file
    fn from_url(url: &Url, base_dir: &Path) -> Result<Self, FetchError> {
        let mut url_segments = match url.path_segments() {
            None => {
                return Err(FetchError::PathError(format!(
                    "Provided `url` parameter '{}' does not have path segments",
                    url.as_str()
                )))
            }
            Some(split) => split.map(String::from).collect::<VecDeque<String>>(),
        };

        let last_url_segment = url_segments.pop_back().ok_or_else(|| {
            FetchError::PathError(format!(
                "Path of URL '{}' is empty. Expected at least one path segment",
                url.as_str()
            ))
        })?;
        if !last_url_segment.contains(".") {
            return Err(FetchError::PathError(format!(
                    "Expected the last element in the URL to contain a period (file extension), e.g. '.json' or '.html' but got: '{}'",
                    url.as_str()
                )));
        }
        let mut cache_dir = base_dir.to_owned();
        for segment in url_segments {
            cache_dir.push(segment);
        }
        Ok(CachePath {
            dir: cache_dir,
            file: last_url_segment,
        })
    }

    /// Return the directory path.
    fn dir(&self) -> &Path {
        &self.dir
    }
    /// Return the file name without the directory.
    fn file(&self) -> &str {
        &self.file
    }
    /// Return the full path to the file, using `self.dir` as the parent folder.
    fn path(&self) -> PathBuf {
        self.dir.join(&self.file)
    }

    /// Return whether the file exists.
    fn exists(&self) -> bool {
        self.path().exists()
    }

    /// Create the directory if it does not exist.
    fn create_dir(&self) -> Result<(), FetchError> {
        fs::create_dir_all(&self.dir)?;
        Ok(())
    }
}

/// An HTTP client that caches responses.
pub struct CachedClient<'a> {
    base_cache_dir: PathBuf,
    client: &'a Client,
}

impl<'a> CachedClient<'a> {
    pub fn new(base_cache_dir: PathBuf, client: &'a Client) -> Self {
        CachedClient {
            base_cache_dir,
            client,
        }
    }

    /// Query the URL and return the response as a string.
    ///
    /// If the response is already cached, return the cached response without querying.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to fetch
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The response body as a string
    /// * `Err(FetchError)` - An error if the request fails or an error occured while creating the cache file or folder
    pub async fn fetch(&self, url: &Url) -> Result<String, FetchError> {
        let cache_path = CachePath::from_url(url, &self.base_cache_dir)?;
        if cache_path.exists() {
            return Ok(tokio::fs::read_to_string(&cache_path.path()).await?);
        }
        let response = self.client.get(url.clone()).send().await?.text().await?;
        cache_path.create_dir()?;
        tokio::fs::write(cache_path.path(), &response).await?;
        Ok(response)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    // Assert the result is an error whose message matches the given regex.
    macro_rules! assert_err {
        ($result:expr, $pattern:expr) => {
            match $result {
                Ok(_) => panic!("Expected an error, but got Ok"),
                Err(e) => {
                    if let Some(pattern) = $pattern {
                        let error_str = e.to_string();
                        let pattern_regex = regex::Regex::new(pattern).unwrap();
                        assert!(
                            pattern_regex.is_match(&error_str),
                            "Error message '{}' does not contain expected pattern '{}'",
                            error_str,
                            pattern
                        );
                    }
                }
            }
        };
    }

    mod cache_path {
        use super::*;
        mod from_url {
            use super::*;

            // Test different base directories
            #[rstest]
            #[case::tilde("~/Documents")]
            #[case::epanded_path("/home/jdoe/Documents")]
            #[case::one_level_relative_dir("../foo")]
            #[case::multi_level_relative_dir("./../foo")]
            #[case::current_directory(".")]
            fn test_base_dirs(#[case] base_dir: PathBuf) {
                // Arrange
                let url =
                    Url::parse("https://api.census.gov/data/2020/acs/acs5/variables.json").unwrap();

                // Act
                let cache_path = CachePath::from_url(&url, base_dir.as_path()).unwrap();

                // Assert
                assert_eq!(cache_path.file(), "variables.json");
                assert_eq!(cache_path.dir(), base_dir.join("data/2020/acs/acs5"));
                assert_eq!(
                    cache_path.path().to_str().unwrap(),
                    base_dir
                        .join("data/2020/acs/acs5/variables.json")
                        .to_str()
                        .unwrap()
                );
            }

            // Test different base URLs
            #[rstest]
            #[case::http("http://api.census.gov/data/2020/acs/acs5/variables.json")]
            #[case::https("https://api.census.gov/data/2020/acs/acs5/variables.json")]
            #[case::localhost("http://localhost:8000/data/2020/acs/acs5/variables.json")]
            fn test_urls(#[case] url_str: &str) {
                // Arrange
                let url = Url::parse(url_str).unwrap();
                let base_dir = Path::new(".");

                // Act
                let cache_path = CachePath::from_url(&url, base_dir).unwrap();

                // Assert
                assert_eq!(cache_path.file(), "variables.json");
                assert_eq!(cache_path.dir().to_str().unwrap(), "./data/2020/acs/acs5");
                assert_eq!(
                    cache_path.path().to_str().unwrap(),
                    "./data/2020/acs/acs5/variables.json"
                );
            }

            #[test]
            fn test_no_path() {
                // Arrange
                let url = Url::parse("https://api.census.gov").unwrap();
                let base_dir = Path::new(".");

                // Act
                let result = CachePath::from_url(&url, base_dir);

                // Assert
                assert_err!(result, Some(".*period.*"));
            }

            #[test]
            fn test_no_file_extension() {
                // Arrange
                let url =
                    Url::parse("https://api.census.gov/data/2020/acs/acs5/variables").unwrap();
                let base_dir = Path::new(".");

                // Act
                let result = CachePath::from_url(&url, base_dir);

                // Assert
                assert_err!(result, Some(".*file extension.*"));
            }
        }
    }
}
