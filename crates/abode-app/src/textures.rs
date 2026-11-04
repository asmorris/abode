//! Decoded images for attachments (thumbnails, blueprint underlays).

use abode_core::AttachmentId;
use abode_core::storage::Store;
use eframe::egui;
use std::collections::HashMap;

#[derive(Default)]
pub struct TextureCache {
    map: HashMap<(AttachmentId, u32), Option<egui::TextureHandle>>,
}

impl TextureCache {
    /// Texture for an image attachment, scaled to fit `max_dim`. `None` for
    /// non-images or files that fail to decode (cached, so we don't retry).
    pub fn get(&mut self, ctx: &egui::Context, store: &Store, id: AttachmentId, max_dim: u32) -> Option<egui::TextureHandle> {
        self.map
            .entry((id, max_dim))
            .or_insert_with(|| load(ctx, store, id, max_dim))
            .clone()
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}

fn load(ctx: &egui::Context, store: &Store, id: AttachmentId, max_dim: u32) -> Option<egui::TextureHandle> {
    let meta = store.attachment_meta(id)?;
    if !meta.is_image() {
        return None;
    }
    let bytes = store.read_attachment(id).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let img = if img.width() > max_dim || img.height() > max_dim {
        img.thumbnail(max_dim, max_dim)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(format!("attachment-{id}-{max_dim}"), color, egui::TextureOptions::LINEAR))
}
