//! Buffer diff helpers for partial e-ink updates.
//!
//! Pure logic (no hardware) so it can be unit-tested without flashing.

extern crate alloc;

use alloc::vec::Vec;

/// Diff result in byte coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffRegion {
    pub min_x_byte: usize,
    pub max_x_byte: usize,
    pub min_y: usize,
    pub max_y: usize,
    pub changed: usize,
}

impl DiffRegion {
    pub fn width_bytes(self) -> usize {
        self.max_x_byte.saturating_sub(self.min_x_byte) + 1
    }

    pub fn height(self) -> usize {
        self.max_y.saturating_sub(self.min_y) + 1
    }

    pub fn byte_count(self) -> usize {
        self.width_bytes() * self.height()
    }

    pub fn x_px(self) -> u16 {
        (self.min_x_byte * 8) as u16
    }

    pub fn w_px(self) -> u16 {
        (self.width_bytes() * 8) as u16
    }

    pub fn y_px(self) -> u16 {
        self.min_y as u16
    }

    pub fn h_px(self) -> u16 {
        self.height() as u16
    }
}

/// Compute the minimal bounding region of changes.
/// Returns None if buffers are identical or sizes are inconsistent.
pub fn compute_diff_region(
    current: &[u8],
    last: &[u8],
    width_bytes: usize,
    height: usize,
) -> Option<DiffRegion> {
    let expected_len = width_bytes * height;
    if current.len() != expected_len || last.len() != expected_len {
        return None;
    }

    let mut min_x = usize::MAX;
    let mut max_x = 0usize;
    let mut min_y = usize::MAX;
    let mut max_y = 0usize;
    let mut changed = 0usize;

    for (i, (&new_b, &old_b)) in current.iter().zip(last.iter()).enumerate() {
        if new_b != old_b {
            changed += 1;
            let y = i / width_bytes;
            let x = i % width_bytes;
            if x < min_x {
                min_x = x;
            }
            if x > max_x {
                max_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if y > max_y {
                max_y = y;
            }
        }
    }

    if changed == 0 {
        None
    } else {
        Some(DiffRegion {
            min_x_byte: min_x,
            max_x_byte: max_x,
            min_y,
            max_y,
            changed,
        })
    }
}

/// Extract a compact region buffer from a full buffer.
pub fn extract_region(source: &[u8], width_bytes: usize, region: DiffRegion, out: &mut Vec<u8>) {
    let region_width = region.width_bytes();
    let region_height = region.height();
    let region_bytes = region_width * region_height;

    out.clear();
    out.resize(region_bytes, 0xFF);

    for row in 0..region_height {
        let src = (region.min_y + row) * width_bytes + region.min_x_byte;
        let dst = row * region_width;
        out[dst..dst + region_width].copy_from_slice(&source[src..src + region_width]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_none_for_equal_buffers() {
        let buf = vec![0xFFu8; 16];
        let region = compute_diff_region(&buf, &buf, 4, 4);
        assert!(region.is_none());
    }

    #[test]
    fn diff_single_byte() {
        let a = vec![0xFFu8; 16];
        let mut b = a.clone();
        b[5] = 0x00;
        let region = compute_diff_region(&b, &a, 4, 4).unwrap();
        assert_eq!(region.min_x_byte, 1);
        assert_eq!(region.max_x_byte, 1);
        assert_eq!(region.min_y, 1);
        assert_eq!(region.max_y, 1);
        assert_eq!(region.changed, 1);
    }

    #[test]
    fn extract_region_bytes() {
        let mut buf = vec![0xFFu8; 16];
        buf[5] = 0x00;
        let region = compute_diff_region(&buf, &vec![0xFFu8; 16], 4, 4).unwrap();
        let mut out = Vec::new();
        extract_region(&buf, 4, region, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], 0x00);
    }

    #[test]
    fn diff_matches_expected_region() {
        // 8x4 pixels => width_bytes = 1, height = 4
        // Change two bytes in different rows
        let prev = vec![0xFFu8; 4];
        let mut next = prev.clone();
        next[0] = 0x7F; // change in row 0
        next[2] = 0x00; // change in row 2

        let region = compute_diff_region(&next, &prev, 1, 4).unwrap();
        assert_eq!(region.min_x_byte, 0);
        assert_eq!(region.max_x_byte, 0);
        assert_eq!(region.min_y, 0);
        assert_eq!(region.max_y, 2);
        assert_eq!(region.byte_count(), 3);

        let mut out = Vec::new();
        extract_region(&next, 1, region, &mut out);
        assert_eq!(out, vec![0x7F, 0xFF, 0x00]);
    }
}
