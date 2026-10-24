use std::str::FromStr;

use super::PackageCoordinate;

#[test]
fn default_extension_is_not_rendered() {
    let coordinate = PackageCoordinate::from_str("net.example:client:1.0").unwrap();

    assert_eq!(coordinate.extension(), "jar");
    assert_eq!(coordinate.to_string(), "net.example:client:1.0");
}
