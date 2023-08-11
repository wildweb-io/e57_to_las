use e57::Point as OriginalPoint;

pub struct ExtendedPoint {
    pub original_point: OriginalPoint,
    pub rgb_color: las::Color,
}

impl From<OriginalPoint> for ExtendedPoint {
    fn from(point: OriginalPoint) -> Self {
        let rgb_color = if let (Some(col), None) = (&point.color, point.color_invalid) {
            las::Color {
                red: to_u16(col.red),
                green: to_u16(col.green),
                blue: to_u16(col.blue),
            }
        } else {
            las::Color::default()
        };

        ExtendedPoint {
            original_point: point,
            rgb_color,
        }
    }
}

pub fn clamp(value: f32) -> f32 {
    if value < 0.0 {
        0.0
    } else if value > 1.0 {
        1.0
    } else {
        value
    }
}

fn to_u16(value: f32) -> u16 {
    (clamp(value) * 255.0) as u16
}
