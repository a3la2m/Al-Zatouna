# PRINTING — Print management design (A4 / Arabic RTL PDF)
## Education Center Management System (Phase 1)

Printing is a first-class module. Every official document is A4 by default, RTL Arabic, with embedded fonts, page headers/footers, and versioned templates. Preview, PDF and paper print must match.

---

## 1. Engine choice: Typst (embedded, in-process)

- **PDF generation inside the Rust process** using the `typst` crate (compiled runtime): templates are `.typ` sources, the document body is the Rust-produced typed JSON payload.
- Strong advantages for this system:
  - native bidirectional text + Arabic shaping and `dir: rtl` support,
  - automatic font subsetting/embedding (Cairo or IBM Plex Sans Arabic bundled inside the app),
  - precise A4 page setup, margins, repeating table headers (`table.header(repeat: true)`), page numbering (`counter(page)`), footers/headers with logos,
  - deterministic, vector, print-accurate output — identical for preview, PDF and printer,
  - no browser print quirks and no third-party popups; no external servers or services.
- **Preview**: the same PDF is rendered to images (multi-page, zoomable) inside the frontend Print Preview screen — the preview literally IS the PDF, so preview/printed/PDF matching is by construction. Alternative renderers: `pdfium` via `pdfium-render` (chosen) for raster preview and print spooling.
- **Excel exports** use `rust_xlsxwriter` (report sheets with RTL sheet-view flag where needed).

## 2. Page defaults (all official documents)

- Paper: A4 (210mm × 297mm), Portrait default; landscape allowed for schedule-wide reports.
- Margins: 15mm default (customizable per print profile).
- Direction: RTL; fonts: Cairo (default) or IBM Plex Sans Arabic; font size 10–11pt body.
- Header: center name + logo + contact block + licence/tax number + custom text; footer: contact line, legal/admin text, signature blocks, page X/Y, print date/time, report title.
- Table behavior: header row repeats on every page; a row is never split across pages; totals row pinned per page and grand total at the end.

## 3. Templates and versioning

- Template sources live in `<data>/templates/<group>/<template>.typ` where group = receipts | reports | certificates.
- App ships built-in default templates; users with the right permission may duplicate and edit copies (text placement, footer lines, signature positions, watermark, hide/show header).
- Every template source carries a semantic `version` and a hash; **each printed document records the template code + version + payload snapshot** in `document_print_log`, and the exact payload used (`payload.json` copy for financial documents like vouchers and decisions) — so reprinting an old voucher reproduces the exact original look with today's template-availability state.
- Reprint rule: reprinting selects the SAME voucher record; no new number is ever minted by a reprint. Print count is incremented per log entry.

## 4. Required template set (V1)

- Receipt voucher, Payment voucher
- Enrollment receipt, Student enrollment form
- Student statement, Teacher statement, Teacher entitlements statement
- Attendance sheet, Study schedule, Sessions schedule
- Course completion certificate, Participation certificate
- Refund request, Shortfall decision, Course settlement statement
- Register closing memo
- Revenue report, Expenses report and the other financial/administrative report set

## 5. Print profiles

`print_profiles` rows bound per document type (code like `voucher.receipt`, `report.revenue`, `certificate.completion`) with: paper, orientation, margins, font-family/size, show-header, show-footer, watermark, copies, language direction. Defaults are seeded; the UI allows creation/copying; a profile drives all rendering parameters passed to the engine.

## 6. Flow and background behavior

- User triggers print/preview → Rust command validates permission + template existence → background Tokio task:
  1. loads the data payload,
  2. composes typst source (template + payload),
  3. compiles PDF into `exports/pdf/<doc_type>/<doc_id>_<timestamp>.pdf`,
  4. records document_print_log (+ checksum),
  5. emits progress events, then returns either the PDF bytes (for preview print) or the stored path.
- Preview UI: page navigation, zoom, choose printer/copies/color mode, print directly (PDF spooling via OS), download PDF, reprint historical documents.
- Print spooling path (local OS printing) uses the OS PDF-printer dialog to avoid duplicating printing subsystems; the PDF is already A4-exact.
- The same pipeline drives **certificates** and **bulk letter/certificate runs** with per-page data; and export runs to folder for archiving.

## 7. Arabic/RTL test checklist (must pass; details in TEST_PLAN §6)

- Arabic glyph shaping + correct bidi mixing with numbers/latin inside the text.
- Tashkeel rendering (optional marking in letters) not corrupting layout.
- Long Arabic strings with no overflow to page edge; text wrapping rules by word.
- Multi-page accounting statements with page x/y plus totals phrases (e.g. "الإجمالي", "إجمالي الصفحة", "المحصّل").
- Header/footer visible on every page, correct page numbers, print date.
- Font embedded: PDF opens on a machine with no installed Arabic fonts and still displays identically.
