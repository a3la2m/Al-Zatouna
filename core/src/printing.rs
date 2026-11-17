//! Printing (Phase 8): Typst-based A4 PDF generation.


use std::collections::HashMap;

use typst::diag::FileResult;
use typst::foundations::{Bytes, Datetime, Duration};
use typst::{Library, LibraryExt};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::World;

pub struct TypstWorld {
    main: FileId,
    source: Source,
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    assets: HashMap<String, Bytes>,
}

impl TypstWorld {
    pub fn new(template: &str) -> Self {
        Self::new_with_assets(template, &[])
    }

    /// Build a world that can serve embedded assets (e.g. the center logo)
    /// referenced from the template as `#image("logo.png")`.
    pub fn new_with_assets(template: &str, assets: &[(&str, Vec<u8>)]) -> Self {
        let main = FileId::new(RootedPath::new(VirtualRoot::Project, VirtualPath::new("main.typ").unwrap()));
        let source = Source::new(main, template.to_string());
        let library = LazyHash::new(Library::default());
        let mut book = FontBook::new();
        let mut fonts = Vec::new();
        for bytes in [
            include_bytes!("../assets/fonts/NotoSansArabic-Regular.ttf").as_slice(),
            include_bytes!("../assets/fonts/NotoSansArabic-Bold.ttf").as_slice(),
        ] {
            if let Some(font) = Font::new(Bytes::new(bytes.to_vec()), 0) {
                book.push(font.info().clone());
                fonts.push(font);
            }
        }
        let book = LazyHash::new(book);
        let assets = assets
            .iter()
            .map(|(name, bytes)| (name.to_string(), Bytes::new(bytes.clone())))
            .collect();
        Self { main, source, library, book, fonts, assets }
    }
}

impl World for TypstWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }
    fn main(&self) -> FileId {
        self.main
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            Ok(self.source.clone())
        } else {
            Err(typst::diag::FileError::NotFound(std::path::PathBuf::from(id.vpath().get_with_slash())))
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main {
            Ok(Bytes::from_string(self.source.text().to_string()))
        } else {
            let path = id.vpath().get_with_slash();
            if let Some(bytes) = self.assets.get(path) {
                return Ok(bytes.clone());
            }
            // also try just the file name (when the asset is referenced as "logo.png")
            let name = path.rsplit('/').next().unwrap_or(&path);
            if let Some(bytes) = self.assets.get(name) {
                return Ok(bytes.clone());
            }
            Err(typst::diag::FileError::NotFound(std::path::PathBuf::from(path)))
        }
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

/// Render a Typst template to PDF bytes.
pub fn render_pdf(template: &str) -> Result<Vec<u8>, String> {
    let world = TypstWorld::new(template);
    let doc = typst::compile::<typst_layout::PagedDocument>(&world).output.map_err(|e| format!("{:?}", e))?;
    let pdf = typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default())
        .map_err(|e| format!("{:?}", e))?;
    Ok(pdf)
}

/// Render a Typst template to PDF bytes with embedded assets (logo etc).
pub fn render_pdf_with_assets(
    template: &str,
    assets: &[(&str, Vec<u8>)],
) -> Result<Vec<u8>, String> {
    let world = TypstWorld::new_with_assets(template, assets);
    let doc = typst::compile::<typst_layout::PagedDocument>(&world).output.map_err(|e| format!("{:?}", e))?;
    let pdf = typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default())
        .map_err(|e| format!("{:?}", e))?;
    Ok(pdf)
}

/// Render a Typst template to PNG (first page) with embedded assets.
pub fn render_png_with_assets(template: &str, assets: &[(&str, Vec<u8>)], _scale: f32) -> Result<Vec<u8>, String> {
    let world = TypstWorld::new_with_assets(template, assets);
    let doc = typst::compile::<typst_layout::PagedDocument>(&world).output.map_err(|e| format!("{:?}", e))?;
    let page = doc.pages().first().ok_or("no pages")?;
    let pixmap = typst_render::render(page, &typst_render::RenderOptions::default());
    Ok(pixmap.encode_png().map_err(|e| format!("{:?}", e))?)
}

/// Render a Typst template to PNG (first page) for preview parity tests.
pub fn render_png_first_page(template: &str, _scale: f32) -> Result<Vec<u8>, String> {
    let world = TypstWorld::new(template);
    let doc = typst::compile::<typst_layout::PagedDocument>(&world).output.map_err(|e| format!("{:?}", e))?;
    let page = doc.pages().first().ok_or("no pages")?;
    let pixmap = typst_render::render(page, &typst_render::RenderOptions::default());
    Ok(pixmap.encode_png().map_err(|e| format!("{:?}", e))?)
}

/// Page count helper.
pub fn page_count(template: &str) -> Result<usize, String> {
    let world = TypstWorld::new(template);
    let doc = typst::compile::<typst_layout::PagedDocument>(&world).output.map_err(|e| format!("{:?}", e))?;
    Ok(doc.pages().len())
}


/// Record a print event (template versioning at issuance, FR-12.5).
pub async fn log_print(
    pool: &sqlx::SqlitePool,
    actor_user_id: i64,
    document_type: &str,
    document_id: i64,
    template_code: &str,
    template_version: &str,
    purpose: &str,
    output_path: &str,
    checksum: &str,
) -> Result<i64, sqlx::Error> {
    let res = sqlx::query(
        "INSERT INTO document_print_log (document_type, document_id, template_code, template_version, printed_by, purpose, output_path, checksum)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(document_type)
    .bind(document_id)
    .bind(template_code)
    .bind(template_version)
    .bind(actor_user_id)
    .bind(purpose)
    .bind(output_path)
    .bind(checksum)
    .execute(pool)
    .await?;
    Ok(res.last_insert_rowid())
}
