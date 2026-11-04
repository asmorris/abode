//! Importing existing blueprints to trace over.
//!
//! Importers turn a file into an image that is shown under the plan. Raster
//! images are supported now; a PDF importer (rasterising a page) or an
//! automatic room-detection importer can be added by implementing
//! [`BlueprintImporter`] and listing it in [`importers`].

use anyhow::{Context, Result};
use std::io::Cursor;
use std::path::Path;

pub struct ImportedBlueprint {
    pub filename: String,
    /// Encoded image bytes (stored as an attachment).
    pub bytes: Vec<u8>,
    pub pixel_size: [u32; 2],
}

pub trait BlueprintImporter {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn import(&self, path: &Path) -> Result<ImportedBlueprint>;
}

pub struct RasterImageImporter;

impl BlueprintImporter for RasterImageImporter {
    fn name(&self) -> &'static str {
        "Image"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["png", "jpg", "jpeg", "gif", "bmp", "webp"]
    }

    fn import(&self, path: &Path) -> Result<ImportedBlueprint> {
        let bytes = std::fs::read(path).with_context(|| format!("couldn't read {}", path.display()))?;
        let (w, h) = image::ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()?
            .into_dimensions()
            .context("not a supported image")?;
        Ok(ImportedBlueprint {
            filename: path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| "blueprint".into()),
            bytes,
            pixel_size: [w, h],
        })
    }
}

pub fn importers() -> Vec<Box<dyn BlueprintImporter>> {
    vec![Box::new(RasterImageImporter)]
}

pub fn importer_for(path: &Path) -> Option<Box<dyn BlueprintImporter>> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    importers().into_iter().find(|i| i.extensions().contains(&ext.as_str()))
}

pub fn supported_extensions() -> Vec<&'static str> {
    importers().iter().flat_map(|i| i.extensions().iter().copied()).collect()
}
