use super::*;
use image::{GrayImage, ImageFormat, Luma, Rgba};
use std::io::Cursor;

fn encoded(picture: impl Into<image::DynamicImage>) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    picture
        .into()
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn within(bytes: &[u8], edge: u32) -> Option<RgbaImage> {
    png_within(Cursor::new(bytes), edge).unwrap()
}

#[test]
fn a_picture_is_shown_no_larger_than_the_edge() {
    assert_eq!(
        fit_within(640, 480, 2048),
        (640, 480),
        "small enough already"
    );
    assert_eq!(fit_within(3840, 2160, 2048), (2048, 1152));
    assert_eq!(
        fit_within(1000, 4000, 2048),
        (512, 2048),
        "tall pictures too"
    );
    assert_eq!(fit_within(10_000, 1, 2048), (2048, 1), "never zero");
}

#[test]
fn an_even_colour_stays_exactly_even_at_an_awkward_ratio() {
    // 1000 → 640 is 1.5625 source pixels per output pixel: every output pixel
    // straddles a source pixel, so any weight that does not sum up shows.
    let colour = Rgba([200, 120, 40, 255]);
    let bytes = encoded(RgbaImage::from_pixel(1000, 700, colour));
    let small = within(&bytes, 640).expect("shrunk");
    assert_eq!(small.dimensions(), (640, 448));
    assert!(small.pixels().all(|pixel| *pixel == colour));
}

#[test]
fn each_output_pixel_is_the_mean_of_the_area_it_covers() {
    // Black and white columns, two of each per output pixel at a quarter size.
    let stripes = RgbaImage::from_fn(400, 100, |x, _| {
        if x % 4 < 2 {
            Rgba([0, 0, 0, 255])
        } else {
            Rgba([255, 255, 255, 255])
        }
    });
    let small = within(&encoded(stripes), 100).expect("shrunk");
    assert_eq!(small.dimensions(), (100, 25));
    assert!(
        small
            .pixels()
            .all(|pixel| *pixel == Rgba([128, 128, 128, 255]))
    );
}

#[test]
fn a_transparent_pixel_does_not_tint_its_neighbour() {
    // Opaque red beside fully transparent blue: the mean is half-covered red.
    let pair = RgbaImage::from_fn(2, 1, |x, _| {
        if x == 0 {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([0, 0, 255, 0])
        }
    });
    let small = within(&encoded(pair), 1).expect("shrunk");
    assert_eq!(*small.get_pixel(0, 0), Rgba([255, 0, 0, 128]));
}

#[test]
fn grey_pictures_come_out_as_rgba() {
    let grey = GrayImage::from_pixel(300, 300, Luma([90]));
    let small = within(&encoded(grey), 100).expect("shrunk");
    assert!(
        small
            .pixels()
            .all(|pixel| *pixel == Rgba([90, 90, 90, 255]))
    );
}

#[test]
fn a_picture_that_already_fits_is_left_to_the_usual_decoder() {
    let bytes = encoded(RgbaImage::new(64, 64));
    assert!(within(&bytes, 64).is_none());
    assert!(png_within(Cursor::new(b"not a png".as_slice()), 64).is_err());
}
