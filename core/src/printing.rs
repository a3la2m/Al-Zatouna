//! Printing (Phase 8): Typst-based A4 PDF generation.

use std::sync::Arc;

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
}

impl TypstWorld {
    pub fn new(template: &str) -> Self {
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
        Self { main, source, library, book, fonts }
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
            Err(typst::diag::FileError::NotFound(id.vpath().as_rooted_path().to_path_buf()))
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main {
            Ok(Bytes::from_string(self.source.text().to_string()))
        } else {
            Err(typst::diag::FileError::NotFound(id.vpath().as_rooted_path().to_path_buf()))
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

/// Render a Typst template to PNG (first page) for preview parity tests.
pub fn render_png_first_page(template: &str, scale: f32) -> Result<Vec<u8>, String> {
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
