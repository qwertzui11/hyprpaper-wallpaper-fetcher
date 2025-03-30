use clap::Parser;
use humantime::parse_duration;
use reqwest::header;
use std::path::PathBuf;
use thiserror::Error;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

#[derive(Parser)]
#[command(name = "Hyprpaper Unsplash Wallpaper")]
#[command(version, about = "Request a wallpaper from https://unsplash.com and sets it to hyprpaper using hyprpaperctl", long_about = None)]
struct CliOptions {
    /// File to use as `api-key`. Must contain a single line.
    #[arg(long)]
    api_key_file: Option<PathBuf>,
    /// `api-key` from https://unsplash.com/developers
    #[arg(long)]
    api_key: Option<String>,
    /// the topics to search unsplash for. By default `wallpapers` is used. Another example is
    /// `wallpapers,space`
    #[arg(long)]
    topics: Option<String>,
    /// Were to save the wallpaper to. By default it is `~/Pictures/hyprpaper-wallpaper.jpeg`
    #[arg(long)]
    target_path: Option<PathBuf>,
    /// Set the count on how often to retry when the unsplash request fails due to a network issue.
    /// [default: 30]
    #[arg(long)]
    retry_count: Option<u32>,
    /// Set the retry timeout on how long to wait before retrying to get a new wallpaper.
    /// [default: 10s]
    #[arg(long, value_parser = parse_duration)]
    retry_timeout: Option<std::time::Duration>,
    /// Set log-severity. Valid values are: `error`, `warn`, `info`, `debug` and `trace`.
    #[arg(short, long)]
    verbose: Option<tracing::Level>,
}

impl std::fmt::Debug for CliOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let masked_api_key = self
            .api_key
            .as_ref()
            .map(|k| '*'.to_string().repeat(k.len()));
        f.debug_struct("CliOptions")
            .field("api_key_file", &self.api_key_file)
            .field("api_key", &masked_api_key)
            .field("retry_count", &self.retry_count)
            .field("retry_timeout", &self.retry_timeout)
            .field("verbose", &self.verbose)
            .finish()
    }
}

#[derive(Error, Debug)]
pub enum WallpaperError {
    #[error("failed to do a request")]
    Reqwest(#[from] reqwest::Error),
    #[error("failed to get config dir")]
    ConfigDir,
    #[error("could not read api key")]
    ReadApiKey(std::io::Error),
    #[error("Unsuccessful request status code")]
    Response(reqwest::StatusCode),
    #[error("hyprpaper failed")]
    Hyprpaper(std::io::Error),
    #[error("could not write image")]
    CouldNotWriteImage(std::io::Error),
    #[error("could set auth token")]
    ParseAuthHeader(reqwest::header::InvalidHeaderValue),
    #[error("failed to get a wallpaper after {0} tries")]
    Failed(u32),
}

type WallpaperResult<T> = Result<T, WallpaperError>;

#[tokio::main]
async fn main() -> WallpaperResult<()> {
    let cli_options = CliOptions::parse();

    if let Some(level) = cli_options.verbose {
        tracing_subscriber::fmt().with_max_level(level).init();
    } else {
        tracing_subscriber::fmt::init();
    }
    debug!("Options: {:?}", cli_options);

    let auth = auth_key(&cli_options)?;
    let retry_count = cli_options.retry_count.unwrap_or(30);
    for _ in 0..retry_count {
        let download = download_photo(&cli_options, &auth).await;
        match download {
            Ok(_) => return Ok(()),
            Err(err) => match err {
                WallpaperError::Reqwest(err) => {
                    warn!(
                        "failed to request wallpaper with error `{:?}`, ignoring",
                        err
                    )
                }
                _ => {
                    error!("got the error {:?}. Cancelling.", err);
                    return Err(err);
                }
            },
        };
        sleep(
            cli_options
                .retry_timeout
                .unwrap_or(std::time::Duration::from_secs(10)),
        )
        .await;
    }
    Err(WallpaperError::Failed(retry_count))
}

async fn download_photo(cli_options: &CliOptions, auth: &str) -> WallpaperResult<()> {
    // https://rust-lang-nursery.github.io/rust-cookbook/web/clients/apis.html
    let default_topic = "wallpapers".to_string();
    let topics = cli_options.topics.as_ref().unwrap_or(&default_topic);
    let request_url =
        format!("https://api.unsplash.com/photos/random?topics={topics}&orientation=landscape");
    debug!("getting {}", request_url);
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        auth.parse().map_err(WallpaperError::ParseAuthHeader)?,
    );
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    let random = do_request(&client, &request_url).await?;
    let random = random.json::<RandomResponse>().await?;
    let photo_url = random.urls.full;
    info!(
        "getting the wallpaper (id: {}) with the description '{}' from the author '{}'. It got liked {} times.",
        random.id,
        random.description.unwrap_or("no description".to_string()),
        random.user.name,
        random.likes
    );
    debug!(
        "Image has the resolution {}x{}. loading url: '{}'",
        random.width, random.height, photo_url
    );
    let photo = do_request(&client, &photo_url).await?;
    let photo = photo.bytes().await?;
    let photo_file = photo_file_path(cli_options)?;
    std::fs::write(photo_file.clone(), photo).map_err(WallpaperError::CouldNotWriteImage)?;
    debug!("setting: {:?}", photo_file);
    let photo_file_str = photo_file.to_str().unwrap();
    exec_hyprpaper("unload", photo_file_str)?;
    exec_hyprpaper("preload", photo_file_str)?;
    exec_hyprpaper("wallpaper", &format!(",{}", photo_file_str))?;
    Ok(())
}

fn exec_hyprpaper(command: &str, arg: &str) -> Result<(), WallpaperError> {
    info!("running `hyprpaper {} {}`", command, arg);
    let result = std::process::Command::new("hyprctl")
        .arg("hyprpaper")
        .arg(command)
        .arg(arg)
        .output()
        .map_err(WallpaperError::Hyprpaper)?;
    debug!("{:?}", result);
    Ok(())
}

async fn do_request(client: &reqwest::Client, url: &str) -> WallpaperResult<reqwest::Response> {
    let response = client.get(url).send().await?;
    let status = response.status();
    if !response.status().is_success() {
        Err(WallpaperError::Response(status))
    } else {
        Ok(response)
    }
}

#[derive(Debug, serde::Deserialize)]
struct User {
    name: String,
}

#[derive(Debug, serde::Deserialize)]
struct RandomResponse {
    id: String,
    description: Option<String>,
    user: User,
    urls: PhotoSrcResponse,
    width: i32,
    height: i32,
    likes: i32,
}

#[derive(Debug, serde::Deserialize)]
struct PhotoSrcResponse {
    full: String,
}

fn default_api_key_file_path() -> WallpaperResult<PathBuf> {
    let config_dir = dirs::config_dir().ok_or(WallpaperError::ConfigDir)?;
    Ok(config_dir.join("unsplash-key"))
}

fn api_key_from_file(cli_options: &CliOptions) -> WallpaperResult<String> {
    let api_file = if let Some(file) = cli_options.api_key_file.clone() {
        file
    } else {
        default_api_key_file_path()?
    };
    debug!("using api-key from file {:?}", api_file);
    let api_key = std::fs::read_to_string(api_file).map_err(WallpaperError::ReadApiKey)?;
    let api_key = api_key.trim().to_owned();
    Ok(api_key)
}

fn auth_key(cli_options: &CliOptions) -> WallpaperResult<String> {
    let key = if let Some(key) = cli_options.api_key.clone() {
        debug!("using api-key from commandline argument");
        key
    } else {
        api_key_from_file(cli_options)?
    };
    Ok(format!("Client-ID {}", key))
}

fn photo_file_path(cli_options: &CliOptions) -> WallpaperResult<PathBuf> {
    if let Some(path) = &cli_options.target_path {
        Ok(path.clone())
    } else {
        let result = dirs::picture_dir().ok_or(WallpaperError::ConfigDir)?;
        let result = result.join("hyprpaper-wallpaper.jpeg");
        let result = std::path::absolute(result).unwrap();
        Ok(result)
    }
}
