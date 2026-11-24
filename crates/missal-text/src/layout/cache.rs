use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::{Arc, Mutex, MutexGuard},
};

use super::{
    style::{AttributedText, ParagraphStyle},
    text_layout::TextLayout,
};

const DEFAULT_ENTRIES: usize = 4096;
const DEFAULT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
pub struct LayoutCache {
    entries: Arc<Mutex<Entries>>,
    speculative: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheUsage {
    pub entries: usize,
    pub bytes: usize,
}

pub(super) enum Lookup {
    Hit(Arc<TextLayout>),
    Rebreak(Probe, Arc<TextLayout>),
    Miss(Probe),
}

pub(super) struct Probe {
    key: u64,
    width: Option<u32>,
}

impl Default for LayoutCache {
    fn default() -> Self {
        Self::new(DEFAULT_ENTRIES, DEFAULT_BYTES)
    }
}

impl LayoutCache {
    pub fn new(entries: usize, bytes: usize) -> Self {
        Self {
            entries: Arc::new(Mutex::new(Entries::new(entries.max(16), bytes))),
            speculative: false,
        }
    }

    pub fn speculative(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            speculative: true,
        }
    }

    pub fn clear(&self) {
        self.lock().clear();
    }

    pub fn usage(&self) -> CacheUsage {
        let entries = self.lock();
        CacheUsage {
            entries: entries.count,
            bytes: entries.bytes,
        }
    }

    pub(super) fn find(
        &self,
        text: &AttributedText,
        paragraph: &ParagraphStyle,
        max_width: Option<f32>,
    ) -> Lookup {
        let probe = Probe {
            key: content_key(text, paragraph),
            width: max_width.map(f32::to_bits),
        };
        let mut entries = self.lock();
        let tick = self.use_tick(&mut entries);
        if let Some(found) = entries.get(&probe, text, paragraph, tick) {
            return Lookup::Hit(found);
        }
        match entries.any_width(probe.key, text, paragraph) {
            Some(template) => Lookup::Rebreak(probe, template),
            None => Lookup::Miss(probe),
        }
    }

    pub(super) fn insert(&self, probe: Probe, layout: Arc<TextLayout>) -> Arc<TextLayout> {
        let mut entries = self.lock();
        let tick = self.use_tick(&mut entries);
        entries.insert(probe, layout, tick)
    }

    fn use_tick(&self, entries: &mut Entries) -> Option<u64> {
        (!self.speculative).then(|| entries.next_tick())
    }

    fn lock(&self) -> MutexGuard<'_, Entries> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn content_key(text: &AttributedText, paragraph: &ParagraphStyle) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    paragraph.hash(&mut hasher);
    hasher.finish()
}

struct Entry {
    width: Option<u32>,
    layout: Arc<TextLayout>,
    bytes: usize,
    last_used: u64,
}

impl Entry {
    fn holds(&self, text: &AttributedText, paragraph: &ParagraphStyle) -> bool {
        self.layout.paragraph == *paragraph && self.layout.source == *text
    }
}

#[derive(Default)]
struct Slot {
    shaping_bytes: usize,
    entries: Vec<Entry>,
}

impl Slot {
    fn bytes(&self) -> usize {
        self.shaping_bytes + self.entries.iter().map(|e| e.bytes).sum::<usize>()
    }
}

struct Entries {
    map: HashMap<u64, Slot>,
    count: usize,
    capacity: usize,
    bytes: usize,
    budget: usize,
    tick: u64,
}

impl Entries {
    fn new(capacity: usize, budget: usize) -> Self {
        Self {
            map: HashMap::new(),
            count: 0,
            capacity,
            bytes: 0,
            budget,
            tick: 0,
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    fn clear(&mut self) {
        self.map.clear();
        self.count = 0;
        self.bytes = 0;
    }

    fn get(
        &mut self,
        probe: &Probe,
        text: &AttributedText,
        paragraph: &ParagraphStyle,
        tick: Option<u64>,
    ) -> Option<Arc<TextLayout>> {
        let entry = self
            .map
            .get_mut(&probe.key)?
            .entries
            .iter_mut()
            .find(|e| e.width == probe.width && e.holds(text, paragraph))?;
        if let Some(tick) = tick {
            entry.last_used = tick;
        }
        Some(entry.layout.clone())
    }

    fn any_width(
        &self,
        key: u64,
        text: &AttributedText,
        paragraph: &ParagraphStyle,
    ) -> Option<Arc<TextLayout>> {
        self.map
            .get(&key)?
            .entries
            .iter()
            .find(|e| e.holds(text, paragraph))
            .map(|e| e.layout.clone())
    }

    fn insert(
        &mut self,
        probe: Probe,
        layout: Arc<TextLayout>,
        tick: Option<u64>,
    ) -> Arc<TextLayout> {
        if let Some(found) = self.get(&probe, &layout.source, &layout.paragraph, tick) {
            return found;
        }
        let bytes = layout.geometry_bytes();
        let shares = self.map.get(&probe.key).is_some_and(|slot| {
            slot.entries
                .iter()
                .any(|e| e.layout.shares_shaping_with(&layout))
        });
        let shaping = if shares { 0 } else { layout.shaping_bytes() };
        let fits = self.count < self.capacity && self.bytes + bytes + shaping <= self.budget;
        if tick.is_none() && !fits {
            return layout;
        }
        let slot = self.map.entry(probe.key).or_default();
        slot.shaping_bytes += shaping;
        slot.entries.push(Entry {
            width: probe.width,
            layout: layout.clone(),
            bytes,
            last_used: tick.unwrap_or(0),
        });
        self.count += 1;
        self.bytes += bytes + shaping;
        self.trim();
        layout
    }

    fn over(&self) -> bool {
        self.count > self.capacity || self.bytes > self.budget
    }

    fn trim(&mut self) {
        while self.over() && self.count > 1 {
            self.evict();
        }
    }

    fn evict(&mut self) {
        let mut ages: Vec<u64> = self
            .map
            .values()
            .flat_map(|slot| slot.entries.iter().map(|e| e.last_used))
            .collect();
        ages.sort_unstable();
        let cutoff = ages[(ages.len() - 1) / 2];
        for slot in self.map.values_mut() {
            slot.entries.retain(|e| e.last_used > cutoff);
            let mut kept: Vec<&Entry> = Vec::new();
            for entry in &slot.entries {
                if !kept
                    .iter()
                    .any(|k| k.layout.shares_shaping_with(&entry.layout))
                {
                    kept.push(entry);
                }
            }
            slot.shaping_bytes = kept.iter().map(|e| e.layout.shaping_bytes()).sum();
        }
        self.map.retain(|_, slot| !slot.entries.is_empty());
        self.count = self.map.values().map(|slot| slot.entries.len()).sum();
        self.bytes = self.map.values().map(Slot::bytes).sum();
    }
}
