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
    api_key_file: Option<std::path::PathBuf>,
    /// `api-key` from https://unsplash.com/developers
    #[arg(long)]
    api_key: Option<String>,
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
    #[error("hyprctl failed")]
    Hyprctl(std::io::Error),
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

    tracing_subscriber::fmt::init();

    let auth = auth_key(&cli_options)?;
    let retry_count = cli_options.retry_count.unwrap_or(30);
    for _ in 0..retry_count {
        let download = download_photo(&auth).await;
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
        sleep(std::time::Duration::from_secs(10)).await;
    }
    Err(WallpaperError::Failed(retry_count))
}

async fn download_photo(auth: &str) -> WallpaperResult<()> {
    // https://rust-lang-nursery.github.io/rust-cookbook/web/clients/apis.html
    let request_url =
        "https://api.unsplash.com/photos/random?topics=wallpapers&orientation=landscape";
    debug!("getting {}", request_url);
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        auth.parse().map_err(WallpaperError::ParseAuthHeader)?,
    );
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    let random = do_request(&client, request_url).await?;
    let random = random.json::<RandomResponse>().await?;
    let photo_url = random.urls.full;
    info!(
        "getting a wallpaper from '{}' with the resolution {}x{}. url: '{}'",
        random.user.name, random.width, random.height, photo_url
    );
    let photo = do_request(&client, &photo_url).await?;
    let photo = photo.bytes().await?;
    let photo_file = photo_file_path()?;
    std::fs::write(photo_file.clone(), photo).map_err(WallpaperError::CouldNotWriteImage)?;
    debug!("setting: {:?}", photo_file);
    let result = std::process::Command::new("hyprctl")
        .arg("hyprpaper")
        .arg("unload")
        .arg(photo_file.clone())
        .output()
        .map_err(WallpaperError::Hyprctl)?;
    debug!("{:?}", result);
    let result = std::process::Command::new("hyprctl")
        .arg("hyprpaper")
        .arg("preload")
        .arg(photo_file.clone())
        .output()
        .map_err(WallpaperError::Hyprpaper)?;
    debug!("{:?}", result);
    let result = std::process::Command::new("hyprctl")
        .arg("hyprpaper")
        .arg("wallpaper")
        .arg(format!(",{}", photo_file.to_str().unwrap()))
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
    width: i32,
    height: i32,
    user: User,
    urls: PhotoSrcResponse,
}

#[derive(Debug, serde::Deserialize)]
struct PhotoSrcResponse {
    full: String,
}

fn default_api_key_file_path() -> WallpaperResult<PathBuf> {
    let config_dir = dirs::config_dir().ok_or(WallpaperError::ConfigDir)?;
    Ok(config_dir.join("unsplash-key"))
}

fn api_key() -> WallpaperResult<String> {
    let api_file = default_api_key_file_path()?;
    let api_key = std::fs::read_to_string(api_file).map_err(WallpaperError::ReadApiKey)?;
    let api_key = api_key.trim().to_owned();
    Ok(api_key)
}

fn auth_key(cli_options: &CliOptions) -> WallpaperResult<String> {
    api_key().map(|k| format!("Client-ID {}", k))
}

fn photo_file_path() -> WallpaperResult<PathBuf> {
    let result = dirs::picture_dir().ok_or(WallpaperError::ConfigDir)?;
    let result = result.join("hyprpaper-wallpaper.jpeg");
    let result = std::path::absolute(result).unwrap();
    Ok(result)
}
