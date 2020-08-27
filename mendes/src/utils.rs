#[cfg(feature = "static")]
mod file_mod {
    use crate::application::ClientError;
    use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
    use http::StatusCode;
    use std::path::PathBuf;
    use tokio::fs;

    pub async fn file<B>(mut path: PathBuf) -> Result<http::Response<B>, ClientError>
    where
        B: From<Vec<u8>>,
    {
        let mut metadata = fs::metadata(&path)
            .await
            .map_err(|_| ClientError::NotFound)?;
        if metadata.is_dir() {
            path = path.join("index.html");
            metadata = fs::metadata(&path)
                .await
                .map_err(|_| ClientError::NotFound)?;
        }

        let mut builder = http::Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_LENGTH, metadata.len());

        if let Some(mime) = mime_guess::from_path(&path).first() {
            builder = builder.header(CONTENT_TYPE, mime.to_string());
        }

        let bytes = fs::read(path).await.map_err(|_| ClientError::NotFound)?;
        Ok(builder.body(B::from(bytes)).unwrap())
    }
}

#[cfg(feature = "static")]
pub use file_mod::file;
