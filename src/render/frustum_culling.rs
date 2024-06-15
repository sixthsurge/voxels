use glam::{IVec3, Mat4, UVec3, Vec3};

use crate::{
    terrain::{chunk::CHUNK_SIZE, position_types::ChunkPos},
    util::size::Size3,
};

/// Manages frustum culling, dividing the world into large regions which are culled first so that
/// individual objects only need to be tested against the frustum if the region they fall into is
/// visible
pub struct FrustumCullingRegions {
    view_frustum: Frustum,
    grid_pos_in_chunks: IVec3,
    grid_size: Size3,
    region_size_in_chunks: Size3,
    regions_visible: Vec<bool>,
}

impl FrustumCullingRegions {
    pub fn new(grid_size: Size3, region_size_in_chunks: Size3) -> Self {
        Self {
            view_frustum: Frustum::default(),
            grid_pos_in_chunks: IVec3::ZERO,
            grid_size,
            region_size_in_chunks,
            regions_visible: vec![false; grid_size.product()],
        }
    }

    /// Called once per frame to update the frustum culling regions
    pub fn update(&mut self, view_proj_matrix: &Mat4, camera_pos: Vec3) {
        // update view frustum
        self.view_frustum = Frustum::compute_view_frustum(view_proj_matrix);

        // update the grid position
        let grid_center = camera_pos
            .div_euclid(Vec3::splat(CHUNK_SIZE as f32))
            .as_ivec3();
        self.grid_pos_in_chunks = grid_center - self.region_size_in_chunks.as_ivec3() / 2;

        // perform frustum test for each region
        for (x, y, z) in itertools::iproduct!(
            0..self.grid_size.x as i32,
            0..self.grid_size.y as i32,
            0..self.grid_size.z as i32,
        ) {
            let position_in_grid = UVec3::new(x as u32, y as u32, z as u32);

            let region_index = self.grid_size.flatten(position_in_grid);

            let aabb_center = Vec3::splat(CHUNK_SIZE as f32)
                * (self.grid_pos_in_chunks.as_vec3()
                    + (Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5))
                        * self.region_size_in_chunks.as_vec3());
            let aabb_extent =
                0.5 * (self.region_size_in_chunks * Size3::splat(CHUNK_SIZE)).as_vec3();

            self.regions_visible[region_index] = self
                .view_frustum
                .intersects_aabb(&aabb_center, &aabb_extent);
        }
    }

    /// True if the given chunk position is within the view frustum
    pub fn is_chunk_within_frustum(&self, chunk_pos: &ChunkPos) -> bool {
        let position_in_grid = (chunk_pos.as_ivec3() - self.grid_pos_in_chunks)
            .div_euclid(self.region_size_in_chunks.as_ivec3());

        if self
            .grid_size
            .contains_ivec3(position_in_grid)
        {
            // check large frustum culling region first, to skip many frustum tests
            let region_index = self
                .grid_size
                .flatten(position_in_grid.as_uvec3());

            if !self.regions_visible[region_index] {
                return false;
            }
        }

        let aabb_center = Vec3::splat(CHUNK_SIZE as f32) * (chunk_pos.as_vec3() + 0.5);
        let aabb_extent = Vec3::splat(0.5 * CHUNK_SIZE as f32);

        // region is visible (or doesn't exist)
        self.view_frustum
            .intersects_aabb(&aabb_center, &aabb_extent)
    }
}

/// Represents a frustum by its six planes
#[derive(Clone, Debug, Default)]
pub struct Frustum {
    pub left: FrustumPlane,
    pub right: FrustumPlane,
    pub bottom: FrustumPlane,
    pub top: FrustumPlane,
    pub near: FrustumPlane,
    pub far: FrustumPlane,
}

impl Frustum {
    /// Find the view frustum from the view-projection matrix using the Gribb-Hartmann method
    pub fn compute_view_frustum(view_proj_matrix: &Mat4) -> Self {
        // https://www.gamedevs.org/uploads/fast-extraction-viewing-frustum-planes-from-world-view-projection-matrix.pdf

        Self {
            left: FrustumPlane {
                a: view_proj_matrix.x_axis.w + view_proj_matrix.x_axis.x,
                b: view_proj_matrix.y_axis.w + view_proj_matrix.y_axis.x,
                c: view_proj_matrix.z_axis.w + view_proj_matrix.z_axis.x,
                d: view_proj_matrix.w_axis.w + view_proj_matrix.w_axis.x,
            },
            right: FrustumPlane {
                a: view_proj_matrix.x_axis.w - view_proj_matrix.x_axis.x,
                b: view_proj_matrix.y_axis.w - view_proj_matrix.y_axis.x,
                c: view_proj_matrix.z_axis.w - view_proj_matrix.z_axis.x,
                d: view_proj_matrix.w_axis.w - view_proj_matrix.w_axis.x,
            },
            bottom: FrustumPlane {
                a: view_proj_matrix.x_axis.w + view_proj_matrix.x_axis.y,
                b: view_proj_matrix.y_axis.w + view_proj_matrix.y_axis.y,
                c: view_proj_matrix.z_axis.w + view_proj_matrix.z_axis.y,
                d: view_proj_matrix.w_axis.w + view_proj_matrix.w_axis.y,
            },
            top: FrustumPlane {
                a: view_proj_matrix.x_axis.w - view_proj_matrix.x_axis.y,
                b: view_proj_matrix.y_axis.w - view_proj_matrix.y_axis.y,
                c: view_proj_matrix.z_axis.w - view_proj_matrix.z_axis.y,
                d: view_proj_matrix.w_axis.w - view_proj_matrix.w_axis.y,
            },
            near: FrustumPlane {
                a: view_proj_matrix.x_axis.z,
                b: view_proj_matrix.y_axis.z,
                c: view_proj_matrix.z_axis.z,
                d: view_proj_matrix.w_axis.z,
            },
            far: FrustumPlane {
                a: view_proj_matrix.x_axis.w - view_proj_matrix.x_axis.z,
                b: view_proj_matrix.y_axis.w - view_proj_matrix.y_axis.z,
                c: view_proj_matrix.z_axis.w - view_proj_matrix.z_axis.z,
                d: view_proj_matrix.w_axis.w - view_proj_matrix.w_axis.z,
            },
        }
    }

    /// True if the frustum intersects the axis-aligned bounding box defined by `aabb_min` and `aabb_size`
    /// aabb_center: center of the AABB
    /// aabb_extent: half the size of the AABB
    pub fn intersects_aabb(&self, aabb_center: &Vec3, aabb_extent: &Vec3) -> bool {
        // https://learnopengl.com/Guest-Articles/2021/Scene/Frustum-Culling
        if !aabb_plane_test(aabb_center, aabb_extent, &self.right) {
            return false;
        }
        if !aabb_plane_test(aabb_center, aabb_extent, &self.left) {
            return false;
        }
        if !aabb_plane_test(aabb_center, aabb_extent, &self.bottom) {
            return false;
        }
        if !aabb_plane_test(aabb_center, aabb_extent, &self.top) {
            return false;
        }
        if !aabb_plane_test(aabb_center, aabb_extent, &self.near) {
            return false;
        }
        if !aabb_plane_test(aabb_center, aabb_extent, &self.far) {
            return false;
        }

        true
    }
}

/// Represents a plane equation of the form ax + by + cz + d = 0
#[derive(Clone, Debug, Default)]
pub struct FrustumPlane {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
}

/// True if the AABB defined by `aabb_center` and `aabb_extent` is touching or in front of the
/// frustum plane
fn aabb_plane_test(aabb_center: &Vec3, aabb_extent: &Vec3, plane: &FrustumPlane) -> bool {
    let r = aabb_extent.x * plane.a.abs()
        + aabb_extent.y * plane.b.abs()
        + aabb_extent.z * plane.c.abs();
    -r <= plane_distance(aabb_center, plane)
}

fn plane_distance(point: &Vec3, plane: &FrustumPlane) -> f32 {
    plane.a * point.x + plane.b * point.y + plane.c * point.z + plane.d
}
