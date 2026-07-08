use crate::config::GridSideConfig;

/// 摇杆网格顶点系统
#[derive(Debug, Clone)]
pub struct JoystickGrid {
    /// 24 个顶点 (6x4)
    vertices: Vec<(f32, f32)>,
    /// 15 个格子 (5x3)
    cells: Vec<GridCell>,
}

#[derive(Debug, Clone)]
struct GridCell {
    v0: usize, // 顶点索引
    v1: usize,
    v2: usize,
    v3: usize,
}

impl JoystickGrid {
    pub fn new(config: &GridSideConfig) -> Self {
        let vertices: Vec<(f32, f32)> = config
            .vertices
            .iter()
            .map(|v| (v[0], v[1]))
            .collect();

        let mut cells = Vec::with_capacity(15);
        // 5 列 x 3 行
        for row in 0..3 {
            for col in 0..5 {
                let top_left = row * 6 + col;
                cells.push(GridCell {
                    v0: top_left,
                    v1: top_left + 1,
                    v2: top_left + 7,
                    v3: top_left + 6,
                });
            }
        }

        Self { vertices, cells }
    }

    /// 当前摇杆位置选中的格子索引 (0..15), None 表示未选中
    pub fn selected_cell(&self, point: (f32, f32)) -> Option<usize> {
        self.cells
            .iter()
            .position(|cell| {
                let v0 = self.vertices[cell.v0];
                let v1 = self.vertices[cell.v1];
                let v2 = self.vertices[cell.v2];
                let v3 = self.vertices[cell.v3];
                point_in_quad(point, v0, v1, v2, v3)
            })
    }
}

/// 叉积法判定点是否在四边形内
fn cross(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.1 - a.1 * b.0
}

fn point_in_quad(
    p: (f32, f32),
    v0: (f32, f32),
    v1: (f32, f32),
    v2: (f32, f32),
    v3: (f32, f32),
) -> bool {
    let ab = (v1.0 - v0.0, v1.1 - v0.1);
    let ap = (p.0 - v0.0, p.1 - v0.1);
    let bc = (v2.0 - v1.0, v2.1 - v1.1);
    let bp = (p.0 - v1.0, p.1 - v1.1);
    let cd = (v3.0 - v2.0, v3.1 - v2.1);
    let cp = (p.0 - v2.0, p.1 - v2.1);
    let da = (v0.0 - v3.0, v0.1 - v3.1);
    let dp = (p.0 - v3.0, p.1 - v3.1);

    let c1 = cross(ab, ap);
    let c2 = cross(bc, bp);
    let c3 = cross(cd, cp);
    let c4 = cross(da, dp);

    (c1 >= 0.0 && c2 >= 0.0 && c3 >= 0.0 && c4 >= 0.0)
        || (c1 <= 0.0 && c2 <= 0.0 && c3 <= 0.0 && c4 <= 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_cell() {
        let cfg = GridSideConfig {
            vertices: vec![
                [-1.0, -1.0], [-0.6, -1.0], [-0.2, -1.0],
                [0.2, -1.0],  [0.6, -1.0],  [1.0, -1.0],
                [-1.0, -0.33], [-0.6, -0.33], [-0.2, -0.33],
                [0.2, -0.33],  [0.6, -0.33],  [1.0, -0.33],
                [-1.0, 0.33],  [-0.6, 0.33],  [-0.2, 0.33],
                [0.2, 0.33],   [0.6, 0.33],   [1.0, 0.33],
                [-1.0, 1.0],   [-0.6, 1.0],   [-0.2, 1.0],
                [0.2, 1.0],    [0.6, 1.0],    [1.0, 1.0],
            ],
        };
        let grid = JoystickGrid::new(&cfg);
        // 中心点 (0, 0) 应该在格子 G (索引 7: col 2, row 1)
        assert_eq!(grid.selected_cell((0.0, 0.0)), Some(7));
        // 左上角应该在格子 A (索引 0: col 0, row 0)
        assert_eq!(grid.selected_cell((-0.8, -0.8)), Some(0));
        // 右下角应该在格子 O (索引 14: col 4, row 2)
        assert_eq!(grid.selected_cell((0.8, 0.8)), Some(14));
    }
}
