use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::sync::Arc;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use missal_text::{FaceId, Rgba};
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, TextRenderingMode, UnicodeCmap};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect as PdfRect, Ref, Str, TextStr};

use crate::display::{DisplayItem, DisplayList, GlyphRun};
use crate::engine::{Packet, Page};
use crate::fonts;
use crate::image::{DecodedImage, ImageData, ImageFormat, jpeg_info};

const PT_PER_PX: f32 = 0.75;

#[derive(Clone, Debug, Default)]
pub struct PdfOptions {
    pub title: Option<String>,
    pub compress: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PdfStats {
    pub pages: usize,
    pub fonts: Vec<(String, usize, usize)>,
    pub images: usize,
    pub bytes: usize,
}

struct Alloc(i32);

impl Alloc {
    fn next(&mut self) -> Ref {
        self.0 += 1;
        Ref::new(self.0)
    }
}

struct FontPlan {
    face: FaceId,
    name: String,
    remap: subsetter::GlyphRemapper,
    font_ref: Ref,
    glyphs: BTreeSet<u16>,
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(6));
    encoder.write_all(data).expect("in-memory write");
    encoder.finish().expect("in-memory write")
}

fn subset_tag(index: usize) -> String {
    let mut tag = String::new();
    let mut n = index + 7919;
    for _ in 0..6 {
        tag.push((b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    tag
}

fn rgb(color: Rgba) -> (f32, f32, f32) {
    let [r, g, b, _] = color.to_f32();
    (r, g, b)
}

fn image_key(image: &Arc<ImageData>) -> usize {
    Arc::as_ptr(&image.bytes) as *const u8 as usize
}

pub fn write_pages<'a>(pages: impl IntoIterator<Item = &'a Page>, options: &PdfOptions) -> (Vec<u8>, PdfStats) {
    let pages: Vec<&Page> = pages.into_iter().collect();
    let mut alloc = Alloc(0);
    let catalog_id = alloc.next();
    let tree_id = alloc.next();
    let mut pdf = Pdf::new();
    let mut used: BTreeMap<u16, BTreeSet<u16>> = BTreeMap::new();
    let mut images: Vec<Arc<ImageData>> = Vec::new();
    let mut image_index: HashMap<usize, usize> = HashMap::new();
    for page in &pages {
        let list = page.display_list();
        for item in &list.items {
            match item {
                DisplayItem::Glyphs(run) => {
                    let set = used.entry(run.face.0).or_default();
                    for g in list.glyphs(run) {
                        set.insert(g.id as u16);
                    }
                }
                DisplayItem::Image { image, .. } => {
                    let key = image_key(image);
                    if !image_index.contains_key(&key) {
                        image_index.insert(key, images.len());
                        images.push(image.clone());
                    }
                }
                _ => {}
            }
        }
    }
    let mut plans: Vec<FontPlan> = Vec::new();
    for (index, (face, glyphs)) in used.into_iter().enumerate() {
        let face = FaceId(face);
        let mut remap = subsetter::GlyphRemapper::new();
        for g in &glyphs {
            remap.remap(*g);
        }
        let file = missal_text::face(face).file_name;
        let base = file.trim_end_matches(".ttf").replace(['-', ' '], "");
        plans.push(FontPlan {
            face,
            name: format!("{}+{}", subset_tag(index), base),
            remap,
            font_ref: alloc.next(),
            glyphs,
        });
    }
    let image_refs: Vec<Ref> = images.iter().map(|_| alloc.next()).collect();
    let mut stats = PdfStats {
        pages: pages.len(),
        images: images.len(),
        ..PdfStats::default()
    };
    let mut page_ids = Vec::with_capacity(pages.len());
    for page in &pages {
        let page_id = alloc.next();
        let content_id = alloc.next();
        page_ids.push(page_id);
        let content = page_content(page, &plans, &image_index);
        let width = page.size.width * PT_PER_PX;
        let height = page.size.height * PT_PER_PX;
        {
            let mut p = pdf.page(page_id);
            p.media_box(PdfRect::new(0.0, 0.0, width, height));
            p.parent(tree_id);
            p.contents(content_id);
            let mut resources = p.resources();
            {
                let mut fonts_dict = resources.fonts();
                for (i, plan) in plans.iter().enumerate() {
                    fonts_dict.pair(Name(format!("F{i}").as_bytes()), plan.font_ref);
                }
            }
            if !image_refs.is_empty() {
                let mut x = resources.x_objects();
                for (i, r) in image_refs.iter().enumerate() {
                    x.pair(Name(format!("Im{i}").as_bytes()), *r);
                }
            }
        }
        if options.compress {
            let compressed = deflate(&content);
            pdf.stream(content_id, &compressed).filter(Filter::FlateDecode);
        } else {
            pdf.stream(content_id, &content);
        }
    }
    pdf.pages(tree_id)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    pdf.catalog(catalog_id).pages(tree_id);
    if let Some(title) = &options.title {
        let info = alloc.next();
        pdf.document_info(info).title(TextStr(title)).producer(TextStr("Missal"));
    }
    for plan in &plans {
        let sizes = write_font(&mut pdf, &mut alloc, plan);
        stats.fonts.push((plan.name.clone(), plan.glyphs.len(), sizes));
    }
    for (image, id) in images.iter().zip(&image_refs) {
        write_image(&mut pdf, &mut alloc, image, *id);
    }
    let bytes = pdf.finish();
    stats.bytes = bytes.len();
    (bytes, stats)
}

pub fn write_packet(packet: &Packet, options: &PdfOptions) -> (Vec<u8>, PdfStats) {
    write_pages(packet.pages(), options)
}

fn write_font(pdf: &mut Pdf, alloc: &mut Alloc, plan: &FontPlan) -> usize {
    let data = missal_text::face(plan.face).data;
    let metrics = fonts::metrics(plan.face);
    let units = metrics.units_per_em;
    let font = swash::FontRef::from_index(data, 0).expect("bundled fonts parse");
    let glyph_metrics = font.glyph_metrics(&[]);
    let cid_ref = alloc.next();
    let descriptor_ref = alloc.next();
    let file_ref = alloc.next();
    let cmap_ref = alloc.next();
    let subset = subsetter::subset(data, 0, &plan.remap).unwrap_or_else(|_| data.to_vec());
    let compressed = deflate(&subset);
    let name = Name(plan.name.as_bytes());
    let system = SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    };
    pdf.type0_font(plan.font_ref)
        .base_font(name)
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid_ref)
        .to_unicode(cmap_ref);
    let mut widths: Vec<(u16, f32)> = plan
        .glyphs
        .iter()
        .filter_map(|g| {
            let new = plan.remap.get(*g)?;
            Some((new, glyph_metrics.advance_width(*g) * 1000.0 / units))
        })
        .collect();
    widths.sort_by_key(|(cid, _)| *cid);
    {
        let mut cid = pdf.cid_font(cid_ref);
        cid.subtype(CidFontType::Type2)
            .base_font(name)
            .system_info(system)
            .font_descriptor(descriptor_ref)
            .default_width(0.0)
            .cid_to_gid_map_predefined(Name(b"Identity"));
        let mut w = cid.widths();
        let mut i = 0;
        while i < widths.len() {
            let start = widths[i].0;
            let mut run = vec![widths[i].1];
            let mut j = i + 1;
            while j < widths.len() && widths[j].0 == widths[j - 1].0 + 1 {
                run.push(widths[j].1);
                j += 1;
            }
            w.consecutive(start, run);
            i = j;
        }
    }
    let head = fonts::table(data, b"head").unwrap_or_default();
    let read = |at: usize| -> f32 {
        head.get(at..at + 2)
            .map(|b| i16::from_be_bytes([b[0], b[1]]) as f32 * 1000.0 / units)
            .unwrap_or(0.0)
    };
    let bbox = PdfRect::new(read(36), read(38), read(40), read(42));
    let face = missal_text::face(plan.face);
    let mut flags = FontFlags::SYMBOLIC;
    if face.italic {
        flags |= FontFlags::ITALIC;
    }
    pdf.font_descriptor(descriptor_ref)
        .name(name)
        .flags(flags)
        .bbox(bbox)
        .italic_angle(if face.italic { -10.0 } else { 0.0 })
        .ascent(metrics.ascent * 1000.0 / units)
        .descent(-metrics.descent * 1000.0 / units)
        .cap_height(metrics.ascent * 700.0 / units)
        .stem_v(80.0 + (face.weight as f32 - 400.0) / 4.0)
        .font_file2(file_ref);
    pdf.stream(file_ref, &compressed)
        .filter(Filter::FlateDecode)
        .pair(Name(b"Length1"), subset.len() as i32);
    let mut reverse: HashMap<u16, char> = HashMap::new();
    font.charmap().enumerate(|codepoint, glyph| {
        if let Some(c) = char::from_u32(codepoint) {
            let entry = reverse.entry(glyph).or_insert(c);
            if (*entry as u32) > codepoint && !(0xFB50..=0xFEFF).contains(&codepoint) {
                *entry = c;
            }
        }
    });
    let mut cmap = UnicodeCmap::new(Name(b"Missal-UCS"), system);
    for g in &plan.glyphs {
        if let (Some(new), Some(c)) = (plan.remap.get(*g), reverse.get(g)) {
            cmap.pair(new, *c);
        }
    }
    let cmap_data = cmap.finish();
    pdf.cmap(cmap_ref, &cmap_data);
    subset.len()
}

fn write_image(pdf: &mut Pdf, alloc: &mut Alloc, image: &Arc<ImageData>, id: Ref) {
    if image.format == ImageFormat::Jpeg
        && let Some((w, h, components)) = jpeg_info(&image.bytes)
    {
        let mut x = pdf.image_xobject(id, &image.bytes);
        x.filter(Filter::DctDecode);
        x.width(w as i32);
        x.height(h as i32);
        match components {
            1 => x.color_space().device_gray(),
            4 => x.color_space().device_cmyk(),
            _ => x.color_space().device_rgb(),
        };
        x.bits_per_component(8);
        return;
    }
    let Some(decoded) = image.decoded() else {
        let mut x = pdf.image_xobject(id, &[255, 255, 255]);
        x.width(1);
        x.height(1);
        x.color_space().device_rgb();
        x.bits_per_component(8);
        return;
    };
    let DecodedImage { width, height, rgba } = decoded.as_ref();
    let mut rgb_bytes = Vec::with_capacity((*width * *height * 3) as usize);
    let mut alpha = Vec::with_capacity((*width * *height) as usize);
    let mut translucent = false;
    for px in rgba.chunks_exact(4) {
        rgb_bytes.extend_from_slice(&px[..3]);
        alpha.push(px[3]);
        translucent |= px[3] != 255;
    }
    let mask_id = translucent.then(|| alloc.next());
    let compressed = deflate(&rgb_bytes);
    {
        let mut x = pdf.image_xobject(id, &compressed);
        x.filter(Filter::FlateDecode);
        x.width(*width as i32);
        x.height(*height as i32);
        x.color_space().device_rgb();
        x.bits_per_component(8);
        if let Some(mask) = mask_id {
            x.s_mask(mask);
        }
    }
    if let Some(mask) = mask_id {
        let compressed = deflate(&alpha);
        let mut x = pdf.image_xobject(mask, &compressed);
        x.filter(Filter::FlateDecode);
        x.width(*width as i32);
        x.height(*height as i32);
        x.color_space().device_gray();
        x.bits_per_component(8);
    }
}

fn utf16_text(text: &str) -> String {
    text.to_owned()
}

fn page_content(page: &Page, plans: &[FontPlan], images: &HashMap<usize, usize>) -> Vec<u8> {
    let list: &DisplayList = page.display_list();
    let mut content = Content::new();
    let height = page.size.height * PT_PER_PX;
    content.transform([PT_PER_PX, 0.0, 0.0, -PT_PER_PX, 0.0, height]);
    let font_index: HashMap<u16, usize> = plans.iter().enumerate().map(|(i, p)| (p.face.0, i)).collect();
    let mut open_line: Option<u32> = None;
    let mut fill: Option<Rgba> = None;
    for item in &list.items {
        let line = match item {
            DisplayItem::Glyphs(run) => Some(run.line),
            _ => None,
        };
        if open_line.is_some() && open_line != line {
            content.end_marked_content();
            open_line = None;
        }
        match item {
            DisplayItem::Rect { rect, color, .. } => {
                if color.alpha() == 0 || rect.width <= 0.0 || rect.height <= 0.0 {
                    continue;
                }
                if fill != Some(*color) {
                    let (r, g, b) = rgb(*color);
                    content.set_fill_rgb(r, g, b);
                    fill = Some(*color);
                }
                content.rect(rect.x, rect.y, rect.width, rect.height).fill_nonzero();
            }
            DisplayItem::FieldHighlight { .. } => {}
            DisplayItem::Image { rect, image } => {
                if let Some(index) = images.get(&image_key(image)) {
                    content.save_state();
                    content.transform([rect.width, 0.0, 0.0, -rect.height, rect.x, rect.y + rect.height]);
                    content.x_object(Name(format!("Im{index}").as_bytes()));
                    content.restore_state();
                }
            }
            DisplayItem::Glyphs(run) => {
                if open_line.is_none() {
                    let text = list
                        .lines
                        .get(run.line as usize)
                        .map(|l| utf16_text(&l.text))
                        .unwrap_or_default();
                    content
                        .begin_marked_content_with_properties(Name(b"Span"))
                        .properties()
                        .actual_text(TextStr(&text));
                    open_line = Some(run.line);
                }
                let Some(&font) = font_index.get(&run.face.0) else { continue };
                if fill != Some(run.color) {
                    let (r, g, b) = rgb(run.color);
                    content.set_fill_rgb(r, g, b);
                    fill = Some(run.color);
                }
                glyph_run(&mut content, list, run, &plans[font], font);
            }
        }
    }
    if open_line.is_some() {
        content.end_marked_content();
    }
    content.finish().into_vec()
}

fn glyph_run(content: &mut Content, list: &DisplayList, run: &GlyphRun, plan: &FontPlan, font: usize) {
    let glyphs = list.glyphs(run);
    if glyphs.is_empty() {
        return;
    }
    let data = missal_text::face(plan.face).data;
    let metrics = fonts::metrics(plan.face);
    let Some(font_ref) = swash::FontRef::from_index(data, 0) else { return };
    let glyph_metrics = font_ref.glyph_metrics(&[]);
    let size = run.size;
    let skew = run.synthesis.skew.to_radians().tan();
    content.begin_text();
    content.set_font(Name(format!("F{font}").as_bytes()), size);
    if run.synthesis.embolden {
        let (r, g, b) = rgb(run.color);
        content.set_stroke_rgb(r, g, b);
        content.set_line_width(size / 48.0);
        content.set_text_rendering_mode(TextRenderingMode::FillStroke);
    }
    let mut index = 0;
    while index < glyphs.len() {
        let first = glyphs[index];
        content.set_text_matrix([1.0, 0.0, skew, -1.0, first.x, first.y]);
        let mut positioned = content.show_positioned();
        let mut items = positioned.items();
        let mut pen = first.x;
        let mut j = index;
        while j < glyphs.len() && (glyphs[j].y - first.y).abs() < 0.001 {
            let g = glyphs[j];
            if j > index {
                let adjust = (pen - g.x) * 1000.0 / size;
                if adjust.abs() > 0.001 {
                    items.adjust(adjust);
                }
                pen = g.x;
            }
            let cid = plan.remap.get(g.id as u16).unwrap_or(0);
            items.show(Str(&cid.to_be_bytes()));
            pen += glyph_metrics.advance_width(g.id as u16) * size / metrics.units_per_em;
            j += 1;
        }
        items.finish();
        positioned.finish();
        index = j;
    }
    if run.synthesis.embolden {
        content.set_text_rendering_mode(TextRenderingMode::Fill);
    }
    content.end_text();
}
