use crate::{ColorIndex, CoreError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IVec3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl IVec3 {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

/// Dense voxel volume. Value 0 = empty; 1..=255 = palette index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoxelModel {
    size_x: u32,
    size_y: u32,
    size_z: u32,
    /// Row-major: index = x + y * sx + z * sx * sy
    data: Vec<ColorIndex>,
}

impl VoxelModel {
    pub fn new(size_x: u32, size_y: u32, size_z: u32) -> Result<Self> {
        Self::validate_size(size_x, size_y, size_z)?;
        let len = (size_x as usize)
            .checked_mul(size_y as usize)
            .and_then(|v| v.checked_mul(size_z as usize))
            .ok_or(CoreError::InvalidSize(size_x, size_y, size_z))?;
        Ok(Self {
            size_x,
            size_y,
            size_z,
            data: vec![0; len],
        })
    }

    pub fn size(&self) -> (u32, u32, u32) {
        (self.size_x, self.size_y, self.size_z)
    }

    pub fn size_x(&self) -> u32 {
        self.size_x
    }
    pub fn size_y(&self) -> u32 {
        self.size_y
    }
    pub fn size_z(&self) -> u32 {
        self.size_z
    }

    fn validate_size(sx: u32, sy: u32, sz: u32) -> Result<()> {
        if sx == 0 || sy == 0 || sz == 0 || sx > 256 || sy > 256 || sz > 256 {
            return Err(CoreError::InvalidSize(sx, sy, sz));
        }
        Ok(())
    }

    fn index(&self, x: i32, y: i32, z: i32) -> Result<usize> {
        if x < 0
            || y < 0
            || z < 0
            || x as u32 >= self.size_x
            || y as u32 >= self.size_y
            || z as u32 >= self.size_z
        {
            return Err(CoreError::OutOfBounds {
                x,
                y,
                z,
                sx: self.size_x,
                sy: self.size_y,
                sz: self.size_z,
            });
        }
        Ok(x as usize
            + y as usize * self.size_x as usize
            + z as usize * self.size_x as usize * self.size_y as usize)
    }

    pub fn in_bounds(&self, x: i32, y: i32, z: i32) -> bool {
        self.index(x, y, z).is_ok()
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> Result<ColorIndex> {
        Ok(self.data[self.index(x, y, z)?])
    }

    pub fn get_or_empty(&self, x: i32, y: i32, z: i32) -> ColorIndex {
        self.get(x, y, z).unwrap_or(0)
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, color: ColorIndex) -> Result<()> {
        let i = self.index(x, y, z)?;
        self.data[i] = color;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    pub fn resize(&mut self, size_x: u32, size_y: u32, size_z: u32) -> Result<()> {
        Self::validate_size(size_x, size_y, size_z)?;
        let mut next = Self::new(size_x, size_y, size_z)?;
        let copy_x = self.size_x.min(size_x);
        let copy_y = self.size_y.min(size_y);
        let copy_z = self.size_z.min(size_z);
        for z in 0..copy_z {
            for y in 0..copy_y {
                for x in 0..copy_x {
                    let c = self.get(x as i32, y as i32, z as i32)?;
                    next.set(x as i32, y as i32, z as i32, c)?;
                }
            }
        }
        *self = next;
        Ok(())
    }

    pub fn voxel_count(&self) -> usize {
        self.data.iter().filter(|&&c| c != 0).count()
    }

    /// Iterate occupied voxels as (x, y, z, color_index).
    pub fn iter_occupied(&self) -> impl Iterator<Item = (u8, u8, u8, ColorIndex)> + '_ {
        let sx = self.size_x as usize;
        let sy = self.size_y as usize;
        self.data.iter().enumerate().filter_map(move |(i, &c)| {
            if c == 0 {
                return None;
            }
            let z = i / (sx * sy);
            let rem = i % (sx * sy);
            let y = rem / sx;
            let x = rem % sx;
            Some((x as u8, y as u8, z as u8, c))
        })
    }

    pub fn from_sparse(
        size_x: u32,
        size_y: u32,
        size_z: u32,
        voxels: impl IntoIterator<Item = (u8, u8, u8, ColorIndex)>,
    ) -> Result<Self> {
        let mut model = Self::new(size_x, size_y, size_z)?;
        for (x, y, z, c) in voxels {
            if c == 0 {
                continue;
            }
            model.set(x as i32, y as i32, z as i32, c)?;
        }
        Ok(model)
    }
}
