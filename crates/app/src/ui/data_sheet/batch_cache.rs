//! Decoded grid batches kept across frames. Reading a batch hashes and decodes
//! its whole chunk, so repainting the same rows reuses the decoded batch until
//! the table content changes.

use std::sync::Arc;

use plotium_core::data::{SnapshotRead, TableBatch};

/// A visible window spans at most two adjacent chunks. One cache is shared by
/// every table, so memory stays bounded by these batches of the shown table.
const MAX_CACHED_BATCHES: usize = 2;

#[derive(Clone, Default)]
pub(super) struct BatchCache {
    content: Option<SnapshotRead>,
    /// Least recently used first.
    batches: Vec<(usize, Arc<TableBatch>)>,
}

impl BatchCache {
    fn id() -> egui::Id {
        egui::Id::new("plotium_sheet_batch_cache")
    }

    /// The shared cache, emptied when it holds batches of other content.
    pub(super) fn load(ctx: &egui::Context, content: SnapshotRead) -> Self {
        let cache = ctx
            .data_mut(|data| data.get_temp::<Self>(Self::id()))
            .unwrap_or_default();
        Self::for_content(cache, content)
    }

    pub(super) fn store(self, ctx: &egui::Context) {
        ctx.data_mut(|data| data.insert_temp(Self::id(), self));
    }

    /// Drop the decoded batches once no sheet shows them.
    pub(super) fn clear(ctx: &egui::Context) {
        ctx.data_mut(|data| data.remove_temp::<Self>(Self::id()));
    }

    fn for_content(cache: Self, content: SnapshotRead) -> Self {
        if cache.content.as_ref() == Some(&content) {
            cache
        } else {
            Self {
                content: Some(content),
                batches: Vec::new(),
            }
        }
    }

    pub(super) fn get_or_read<E>(
        &mut self,
        index: usize,
        read: impl FnOnce() -> Result<TableBatch, E>,
    ) -> Result<Arc<TableBatch>, E> {
        if let Some(position) = self.batches.iter().position(|(cached, _)| *cached == index) {
            let entry = self.batches.remove(position);
            let batch = Arc::clone(&entry.1);
            self.batches.push(entry);
            return Ok(batch);
        }
        let batch = Arc::new(read()?);
        if self.batches.len() == MAX_CACHED_BATCHES {
            self.batches.remove(0);
        }
        self.batches.push((index, Arc::clone(&batch)));
        Ok(batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plotium_core::data::{ContentHash, RevisionId, TableId};

    fn content() -> SnapshotRead {
        SnapshotRead {
            table: TableId::new(),
            revision: RevisionId::new(),
            fingerprint: ContentHash::of(b"rows"),
        }
    }

    fn batch(row_start: u64) -> TableBatch {
        TableBatch {
            row_start,
            row_ids: Vec::new(),
            columns: Vec::new(),
        }
    }

    #[test]
    fn repeated_reads_decode_each_batch_once_until_content_changes() {
        let first = content();
        let mut reads = 0;
        let mut cache = BatchCache::for_content(BatchCache::default(), first.clone());
        for _ in 0..60 {
            cache
                .get_or_read(0, || {
                    reads += 1;
                    Ok::<_, ()>(batch(0))
                })
                .unwrap();
        }
        assert_eq!(reads, 1);

        let mut cache = BatchCache::for_content(cache, first);
        cache.get_or_read(0, || Err(())).unwrap();

        let mut cache = BatchCache::for_content(cache, content());
        assert!(cache.get_or_read(0, || Err(())).is_err());
    }

    #[test]
    fn cache_keeps_only_the_most_recent_batches() {
        let mut cache = BatchCache::for_content(BatchCache::default(), content());
        for index in 0..3 {
            cache
                .get_or_read(index, || Ok::<_, ()>(batch(index as u64)))
                .unwrap();
        }
        assert_eq!(cache.batches.len(), MAX_CACHED_BATCHES);
        assert!(cache.get_or_read(0, || Err(())).is_err());
        cache.get_or_read(2, || Err(())).unwrap();
    }
}
