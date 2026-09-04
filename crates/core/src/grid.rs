//! Uniform grid broadphase.
//!
//! Every pair test the narrowphase never sees is the point. At two hundred and
//! fifty entities a full sweep is thirty thousand pairs a tick, which is survivable
//! but wasteful; the grid brings it down to the handful of neighbours that could
//! actually touch.
//!
//! The sensor work at v0.5 walks rays through this same structure with a digital
//! differential analyzer [DDA], which is the other reason to build it now rather
//! than start with the quadratic sweep and replace it later.
//!
//! Cells are at least `2 · MAX_ENTITY_RADIUS` across. That bound is what makes
//! centre-only insertion correct: two overlapping circles have centres no further
//! apart than one cell, so they always land in the same or adjacent cells and the
//! three-by-three neighbourhood cannot miss a contact.

use schema::{EntityId, Vec2};

use crate::constants::MAX_ENTITY_RADIUS;

#[derive(Debug, Clone)]
pub struct Grid {
    cell: f32,
    cols: usize,
    rows: usize,
    /// One bucket per cell, in row-major order.
    cells: Vec<Vec<EntityId>>,
}

impl Grid {
    pub fn new(side: f32) -> Self {
        let cell = (2.0 * MAX_ENTITY_RADIUS).max(1.0);
        let cols = (side / cell).ceil() as usize + 1;
        let rows = cols;
        Self {
            cell,
            cols,
            rows,
            cells: vec![Vec::new(); cols * rows],
        }
    }

    fn index_of(&self, p: Vec2) -> usize {
        let cx = ((p.x / self.cell) as isize).clamp(0, self.cols as isize - 1) as usize;
        let cy = ((p.y / self.cell) as isize).clamp(0, self.rows as isize - 1) as usize;
        cy * self.cols + cx
    }

    /// Empty the buckets, keeping their allocations. Called once a tick.
    pub fn clear(&mut self) {
        for c in &mut self.cells {
            c.clear();
        }
    }

    pub fn insert(&mut self, id: EntityId, pos: Vec2) {
        let i = self.index_of(pos);
        self.cells[i].push(id);
    }

    /// Every candidate pair, each exactly once.
    ///
    /// Within a cell, pairs are taken forward only. Across cells, only the four
    /// neighbours after this one in scan order are consulted — east, and the three
    /// below — which covers the full eight-neighbourhood without visiting a pair
    /// from both sides.
    ///
    /// Order is a function of cell index and insertion order, both stable, so the
    /// pair sequence is identical between runs on the same seed.
    pub fn for_each_pair<F: FnMut(EntityId, EntityId)>(&self, mut f: F) {
        const FORWARD: [(isize, isize); 4] = [(1, 0), (-1, 1), (0, 1), (1, 1)];

        for cy in 0..self.rows {
            for cx in 0..self.cols {
                let here = &self.cells[cy * self.cols + cx];
                if here.is_empty() {
                    continue;
                }
                for i in 0..here.len() {
                    for j in (i + 1)..here.len() {
                        f(here[i], here[j]);
                    }
                }
                for (dx, dy) in FORWARD {
                    let nx = cx as isize + dx;
                    let ny = cy as isize + dy;
                    if nx < 0 || ny < 0 || nx >= self.cols as isize || ny >= self.rows as isize {
                        continue;
                    }
                    let there = &self.cells[ny as usize * self.cols + nx as usize];
                    for &a in here {
                        for &b in there {
                            f(a, b);
                        }
                    }
                }
            }
        }
    }

    /// Handles within `radius` of `p`, by cell. The caller still does the exact
    /// distance test; this only narrows the field.
    pub fn query_radius(&self, p: Vec2, radius: f32, out: &mut Vec<EntityId>) {
        out.clear();
        let reach = radius + MAX_ENTITY_RADIUS;
        let min_x = (((p.x - reach) / self.cell) as isize).clamp(0, self.cols as isize - 1);
        let max_x = (((p.x + reach) / self.cell) as isize).clamp(0, self.cols as isize - 1);
        let min_y = (((p.y - reach) / self.cell) as isize).clamp(0, self.rows as isize - 1);
        let max_y = (((p.y + reach) / self.cell) as isize).clamp(0, self.rows as isize - 1);
        for cy in min_y..=max_y {
            for cx in min_x..=max_x {
                out.extend_from_slice(&self.cells[cy as usize * self.cols + cx as usize]);
            }
        }
    }
}
