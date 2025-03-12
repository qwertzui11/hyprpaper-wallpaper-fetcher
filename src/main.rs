use std::path::PathBuf;

use reqwest::header;
use thiserror::Error;
use tokio::time::sleep;

#[derive(Error, Debug)]
pub enum WallpaperError {
    #[error("failed to do a request")]
    Reqwest(#[from] reqwest::Error),
    #[error("failed to get config dir")]
    ConfigDir,
    #[error("could not read api key")]
    ReadApiKey(std::io::Error),
    #[error("hyprctl failed")]
    Hyprctl(std::io::Error),
    #[error("hyprpaper failed")]
    Hyprpaper(std::io::Error),
    #[error("could not write image")]
    CouldNotWriteImage(std::io::Error),
    #[error("could set auth token")]
    ParseAuthHeader(reqwest::header::InvalidHeaderValue),
}

type WallpaperResult<T> = Result<T, WallpaperError>;

#[tokio::main]
async fn main() -> WallpaperResult<()> {
    let auth = auth_key()?;
    let retry_count = 10;
    for _ in 0..retry_count {
        let download = download_photo(&auth).await;
        if download.is_ok() {
            return Ok(());
        }
        sleep(std::time::Duration::from_secs(60)).await;
    }
    // TODO: return error!
    Ok(())
}

async fn download_photo(auth: &str) -> WallpaperResult<()> {
    // https://rust-lang-nursery.github.io/rust-cookbook/web/clients/apis.html
    let request_url =
        "https://api.unsplash.com/photos/random?topics=wallpapers&orientation=landscape";
    println!("getting {}", request_url);
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
    let photo = random.urls.full;
    println!("getting: {}", photo);
    let photo = do_request(&client, &photo).await?;
    let photo = photo.bytes().await?;
    let photo_file = photo_file_path()?;
    std::fs::write(photo_file.clone(), photo).map_err(WallpaperError::CouldNotWriteImage)?;
    println!("setting: {:?}", photo_file);
    println!(
        "{:?}",
        std::process::Command::new("hyprctl")
            .arg("hyprpaper")
            .arg("unload")
            .arg(photo_file.clone())
            .output()
            .map_err(WallpaperError::Hyprctl)?
    );
    println!(
        "{:?}",
        std::process::Command::new("hyprctl")
            .arg("hyprpaper")
            .arg("preload")
            .arg(photo_file.clone())
            .output()
            .map_err(WallpaperError::Hyprpaper)?
    );
    println!(
        "{:?}",
        std::process::Command::new("hyprctl")
            .arg("hyprpaper")
            .arg("wallpaper")
            .arg(format!(",{}", photo_file.to_str().unwrap()))
            .output()
            .map_err(WallpaperError::Hyprpaper)?
    );
    Ok(())
}

async fn do_request(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    let response = client.get(url).send().await?;
    if response.status() != 200 {
        // TODO: do an error!
        println!("random.status(): {}", response.status());
    }
    Ok(response)
}

#[derive(Debug, serde::Deserialize)]
struct RandomResponse {
    #[allow(dead_code)]
    width: i32,
    #[allow(dead_code)]
    height: i32,
    urls: PhotoSrcResponse,
}

#[derive(Debug, serde::Deserialize)]
struct PhotoSrcResponse {
    full: String,
}

fn api_key() -> WallpaperResult<String> {
    let config_dir = dirs::config_dir().ok_or(WallpaperError::ConfigDir)?;
    let api_file = config_dir.join("unsplash-key");
    let api_key = std::fs::read_to_string(api_file).map_err(WallpaperError::ReadApiKey)?;
    let api_key = api_key.trim().to_owned();
    Ok(api_key)
}

fn auth_key() -> WallpaperResult<String> {
    api_key().map(|k| format!("Client-ID {}", k))
}

fn photo_file_path() -> WallpaperResult<PathBuf> {
    let result = dirs::picture_dir().ok_or(WallpaperError::ConfigDir)?;
    let result = result.join("hyprpaper-wallpaper.jpeg");
    let result = std::path::absolute(result).unwrap();
    Ok(result)
}
