type CoordinateKind = i32;

const SHIFT: CoordinateKind = 100;
const BIN: CoordinateKind = 50;
const ZONE_BOUNDARY: CoordinateKind = 180;

enum Zone {
    Left,
    Right,
}

impl From<Zone> for u8 {
    fn from(val: Zone) -> Self {
        use self::Zone::*;

        match val {
            Left => 0,
            Right => 1,
        }
    }
}

pub fn coord_to_hash(latitude: CoordinateKind, longitude: CoordinateKind) -> i32 {
    ((latitude + SHIFT) / BIN) * 1000 + (longitude + SHIFT) / BIN
}

pub fn coord_to_zone(latitude: CoordinateKind, _longitude: CoordinateKind) -> u8 {
    if latitude < ZONE_BOUNDARY {
        Zone::Left.into()
    } else {
        Zone::Right.into()
    }
}
