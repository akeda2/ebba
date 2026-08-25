use crate::document::piece_tree::PieceTree;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRegion {
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_feeds: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct LineIndex {
    total_len: usize,
    regions: Vec<LineRegion>,
}

impl LineIndex {
    pub fn from_piece_tree(tree: &PieceTree) -> Self {
        let regions = tree
            .piece_metadata()
            .into_iter()
            .map(|piece| LineRegion {
                byte_start: piece.byte_start,
                byte_end: piece.byte_end,
                line_feeds: piece.line_feeds,
            })
            .collect::<Vec<_>>();

        Self {
            total_len: tree.len(),
            regions,
        }
    }

    pub fn rebuild_from_tree(&mut self, tree: &PieceTree) {
        *self = Self::from_piece_tree(tree);
    }

    pub fn total_len(&self) -> usize {
        self.total_len
    }

    pub fn regions(&self) -> &[LineRegion] {
        &self.regions
    }

    pub fn known_line_feeds(&self) -> usize {
        self.regions
            .iter()
            .filter_map(|region| region.line_feeds)
            .sum()
    }

    pub fn exact_line_feeds(&self) -> Option<usize> {
        self.regions
            .iter()
            .try_fold(0usize, |acc, region| region.line_feeds.map(|lf| acc + lf))
    }
}
