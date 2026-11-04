use std::{ops::Range, rc::Rc};

use crate::api::SongRow;

use super::filter::ListFilter;

pub(crate) struct VisibleRows<'a> {
    rows: &'a [SongRow],
    indices: Option<Rc<[usize]>>,
}

impl<'a> VisibleRows<'a> {
    pub(crate) fn len(&self) -> usize {
        self.indices
            .as_ref()
            .map_or(self.rows.len(), |indices| indices.len())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn get(&self, index: usize) -> Option<(usize, &'a SongRow)> {
        let underlying = if let Some(indices) = &self.indices {
            *indices.get(index)?
        } else {
            index
        };
        self.rows.get(underlying).map(|row| (underlying, row))
    }

    pub(crate) fn last(&self) -> Option<(usize, &'a SongRow)> {
        self.get(self.len().checked_sub(1)?)
    }

    pub(crate) fn iter(&self) -> VisibleRowsIter<'a> {
        VisibleRowsIter {
            rows: self.rows,
            indices: self.indices.clone(),
            range: 0..self.len(),
        }
    }
}

impl<'a> IntoIterator for VisibleRows<'a> {
    type Item = (usize, &'a SongRow);
    type IntoIter = VisibleRowsIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let range = 0..self.len();
        VisibleRowsIter {
            rows: self.rows,
            indices: self.indices,
            range,
        }
    }
}

pub(crate) struct VisibleRowsIter<'a> {
    rows: &'a [SongRow],
    indices: Option<Rc<[usize]>>,
    range: Range<usize>,
}

impl<'a> VisibleRowsIter<'a> {
    fn row(&self, index: usize) -> (usize, &'a SongRow) {
        let underlying = self
            .indices
            .as_ref()
            .map_or(index, |indices| indices[index]);
        (underlying, &self.rows[underlying])
    }
}

impl<'a> Iterator for VisibleRowsIter<'a> {
    type Item = (usize, &'a SongRow);

    fn next(&mut self) -> Option<Self::Item> {
        self.range.next().map(|index| self.row(index))
    }

    fn nth(&mut self, count: usize) -> Option<Self::Item> {
        self.range.nth(count).map(|index| self.row(index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}

impl ExactSizeIterator for VisibleRowsIter<'_> {}

/// Unfiltered lists are viewed in place; a filter collects matching indices
/// per call, which is cheap next to drawing and has nothing to invalidate.
pub(super) fn visible_rows<'a>(rows: &'a [SongRow], filter: &ListFilter) -> VisibleRows<'a> {
    let indices = (!filter.query.chars().all(char::is_whitespace)).then(|| {
        rows.iter()
            .enumerate()
            .filter(|(_, row)| filter.matches(row))
            .map(|(index, _)| index)
            .collect()
    });
    VisibleRows { rows, indices }
}
