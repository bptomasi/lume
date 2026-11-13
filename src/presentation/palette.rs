//! Theme colors used while preparing transparent image surfaces.

pub fn image_background(name: &str) -> [u8; 3] {
    match name {
        "nord" => [52, 60, 75],
        "gruvbox" => [40, 40, 40],
        _ => [23, 23, 26],
    }
}
