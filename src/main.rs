use std::env;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::fmt::{self, Write as FmtWrite};
use std::io::{self, Cursor, Read, Write};
use std::os::unix::io::AsRawFd;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::ImageFormat;
use libc::S_IFCHR;
use objc::runtime::{Class, Object, BOOL, YES};
use objc::{msg_send, sel, sel_impl};
use objc_foundation::{INSData, INSString, NSData, NSString};
use objc_id::Id;

const TEXT_PASTEBOARD_TYPES: [&str; 2] = ["public.utf8-plain-text", "NSStringPboardType"];
const TIFF_PASTEBOARD_TYPES: [&str; 2] = ["public.tiff", "NSTIFFPboardType"];
const SVG_PASTEBOARD_TYPES: [&str; 2] = ["public.svg-image", "image/svg+xml"];
const SIXEL_COLOR_LEVELS: usize = 6;
const SIXEL_PALETTE_SIZE: usize = SIXEL_COLOR_LEVELS * SIXEL_COLOR_LEVELS * SIXEL_COLOR_LEVELS;
const SIXEL_TRANSPARENT: u8 = u8::MAX;
const SIXEL_ALPHA_THRESHOLD: u8 = 128;

#[repr(C)]
struct Stat {
    st_dev: i32,
    st_mode: u16,
    st_nlink: u16,
    st_ino: u64,
    st_uid: u32,
    st_gid: u32,
    st_rdev: i32,
    st_atimespec: [u64; 2],
    st_mtimespec: [u64; 2],
    st_ctimespec: [u64; 2],
    st_birthtimespec: [u64; 2],
    st_size: i64,
    st_blocks: i64,
    st_blksize: i32,
    st_flags: u32,
    st_gen: u32,
    st_lspare: i32,
    st_qspare: [i64; 2],
}

extern "C" {
    fn fstat(fd: i32, buf: *mut Stat) -> i32;
    fn realpath(path: *const i8, resolved_path: *mut i8) -> *mut i8;
}

#[derive(Debug)]
struct CocoaClassError {
    class_name: &'static str,
    details: String,
}

impl CocoaClassError {
    fn new(class_name: &'static str, msg: &str) -> CocoaClassError {
        CocoaClassError {
            class_name,
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for CocoaClassError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.class_name, self.details)
    }
}

impl Error for CocoaClassError {}

#[derive(Debug, Eq, PartialEq)]
enum ClipboardAction {
    Copy,
    Paste,
}

enum ClipboardContent {
    Text(Vec<u8>),
    Image(Vec<u8>),
    Svg(Vec<u8>),
    Unknown,
}

#[derive(Debug, Eq, PartialEq)]
enum TerminalImageProtocol {
    Kitty,
    Sixel,
}

fn action_for_stdin(stdin_is_terminal: bool) -> ClipboardAction {
    if stdin_is_terminal {
        ClipboardAction::Paste
    } else {
        ClipboardAction::Copy
    }
}

fn stdin_is_terminal() -> bool {
    unsafe { libc::isatty(io::stdin().as_raw_fd()) == 1 }
}

fn cocoa_class(class_name: &'static str) -> Result<&'static Class, Box<dyn Error>> {
    Class::get(class_name).ok_or_else(|| {
        Box::new(CocoaClassError::new(class_name, "class not found")) as Box<dyn Error>
    })
}

fn stdout_output_device() -> &'static str {
    let mut statbuf = Stat {
        st_dev: 0,
        st_mode: 0,
        st_nlink: 0,
        st_ino: 0,
        st_uid: 0,
        st_gid: 0,
        st_rdev: 0,
        st_atimespec: [0; 2],
        st_mtimespec: [0; 2],
        st_ctimespec: [0; 2],
        st_birthtimespec: [0; 2],
        st_size: 0,
        st_blocks: 0,
        st_blksize: 0,
        st_flags: 0,
        st_gen: 0,
        st_lspare: 0,
        st_qspare: [0; 2],
    };

    unsafe {
        fstat(io::stdout().as_raw_fd(), &mut statbuf);
    }

    if statbuf.st_mode & libc::S_IFMT == libc::S_IFREG {
        return "file";
    } else if statbuf.st_mode & libc::S_IFMT == S_IFCHR {
        return "terminal";
    }
    "unknown"
}

fn get_stdout_filename_extension() -> Result<String, &'static str> {
    let mut resolved_path = vec![0i8; 1024];
    let path = CString::new("/dev/fd/1").unwrap();

    unsafe {
        if realpath(path.as_ptr(), resolved_path.as_mut_ptr()).is_null() {
            return Err("Error calling libc.realpath");
        }
    }

    let filename = unsafe { CStr::from_ptr(resolved_path.as_ptr()) }
        .to_str()
        .unwrap();

    if let Some(period) = filename.rfind('.') {
        return Ok(filename[period + 1..].to_string());
    }

    Ok(String::new())
}

fn general_pasteboard() -> Result<*mut Object, Box<dyn Error>> {
    unsafe {
        let pb: *mut Object = msg_send![cocoa_class("NSPasteboard")?, generalPasteboard];
        if pb.is_null() {
            return Err(Box::new(CocoaClassError::new(
                "NSPasteboard",
                "generalPasteboard returned null",
            )));
        }
        Ok(pb)
    }
}

fn pasteboard_string_for_types(
    pb: *mut Object,
    pasteboard_types: &[&str],
) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    for pasteboard_type in pasteboard_types {
        let ns_type = NSString::from_str(pasteboard_type);
        let content: *mut NSString = unsafe { msg_send![pb, stringForType: &*ns_type] };

        if !content.is_null() {
            let content: Id<NSString> = unsafe { Id::from_ptr(content) };
            return Ok(Some(content.as_str().as_bytes().to_vec()));
        }
    }

    Ok(None)
}

fn pasteboard_data_for_types(
    pb: *mut Object,
    pasteboard_types: &[&str],
) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    for pasteboard_type in pasteboard_types {
        let ns_type = NSString::from_str(pasteboard_type);
        let data: *mut NSData = unsafe { msg_send![pb, dataForType: &*ns_type] };

        if !data.is_null() {
            let data: Id<NSData> = unsafe { Id::from_ptr(data) };
            return Ok(Some(data.bytes().to_vec()));
        }
    }

    Ok(None)
}

fn get_clipboard_content() -> Result<ClipboardContent, Box<dyn Error>> {
    let pb = general_pasteboard()?;

    if let Some(svg) = pasteboard_data_for_types(pb, &SVG_PASTEBOARD_TYPES)? {
        return Ok(ClipboardContent::Svg(svg));
    }

    if let Some(svg) = pasteboard_string_for_types(pb, &SVG_PASTEBOARD_TYPES)? {
        return Ok(ClipboardContent::Svg(svg));
    }

    if let Some(text) = pasteboard_string_for_types(pb, &TEXT_PASTEBOARD_TYPES)? {
        if is_svg_content(&text) {
            return Ok(ClipboardContent::Svg(text));
        }

        return Ok(ClipboardContent::Text(text));
    }

    if let Some(image) = pasteboard_data_for_types(pb, &TIFF_PASTEBOARD_TYPES)? {
        return Ok(ClipboardContent::Image(image));
    }

    Ok(ClipboardContent::Unknown)
}

fn set_pasteboard_text(text: &str) -> Result<(), Box<dyn Error>> {
    let pb = general_pasteboard()?;
    let text = NSString::from_str(text);
    let _: isize = unsafe { msg_send![pb, clearContents] };

    let mut did_write = false;
    for pasteboard_type in &TEXT_PASTEBOARD_TYPES {
        let ns_type = NSString::from_str(pasteboard_type);
        let success: BOOL = unsafe { msg_send![pb, setString: &*text forType: &*ns_type] };
        did_write |= success == YES;
    }

    if did_write {
        Ok(())
    } else {
        Err(Box::new(CocoaClassError::new(
            "NSPasteboard",
            "failed to write text",
        )))
    }
}

fn set_pasteboard_tiff(tiff_data: Vec<u8>) -> Result<(), Box<dyn Error>> {
    let pb = general_pasteboard()?;
    let data = NSData::from_vec(tiff_data);
    let _: isize = unsafe { msg_send![pb, clearContents] };

    let mut did_write = false;
    for pasteboard_type in &TIFF_PASTEBOARD_TYPES {
        let ns_type = NSString::from_str(pasteboard_type);
        let success: BOOL = unsafe { msg_send![pb, setData: &*data forType: &*ns_type] };
        did_write |= success == YES;
    }

    if did_write {
        Ok(())
    } else {
        Err(Box::new(CocoaClassError::new(
            "NSPasteboard",
            "failed to write image",
        )))
    }
}

fn set_pasteboard_svg(svg_data: Vec<u8>) -> Result<(), Box<dyn Error>> {
    let pb = general_pasteboard()?;
    let svg_text = std::str::from_utf8(&svg_data)?;
    let text = NSString::from_str(svg_text);
    let data = NSData::from_vec(svg_data);
    let _: isize = unsafe { msg_send![pb, clearContents] };

    let mut svg_written = false;
    for pasteboard_type in &SVG_PASTEBOARD_TYPES {
        let ns_type = NSString::from_str(pasteboard_type);
        let success: BOOL = unsafe { msg_send![pb, setData: &*data forType: &*ns_type] };
        svg_written |= success == YES;
    }

    for pasteboard_type in &TEXT_PASTEBOARD_TYPES {
        let ns_type = NSString::from_str(pasteboard_type);
        let _: BOOL = unsafe { msg_send![pb, setString: &*text forType: &*ns_type] };
    }

    if svg_written {
        Ok(())
    } else {
        Err(Box::new(CocoaClassError::new(
            "NSPasteboard",
            "failed to write SVG",
        )))
    }
}

fn transform_content(extension: &str, content: &[u8]) -> Option<Vec<u8>> {
    let formats = [
        (vec!["png"], ImageFormat::Png),
        (vec!["jpg", "jpeg"], ImageFormat::Jpeg),
        (vec!["gif"], ImageFormat::Gif),
        (vec!["bmp"], ImageFormat::Bmp),
        (vec!["webp"], ImageFormat::WebP),
    ];

    if !extension.is_empty() {
        for (exts, format) in formats.iter() {
            if exts.contains(&extension.to_lowercase().as_str()) {
                if let Ok(img) = image::load_from_memory_with_format(content, ImageFormat::Tiff) {
                    let mut output = Cursor::new(Vec::new());
                    if img.write_to(&mut output, *format).is_ok() {
                        return Some(output.into_inner());
                    }
                }
            }
        }
    }

    None
}

fn tiff_from_image_data(content: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(content).ok()?;
    let mut output = Cursor::new(Vec::new());

    if img.write_to(&mut output, ImageFormat::Tiff).is_ok() {
        Some(output.into_inner())
    } else {
        None
    }
}

fn starts_with_ignore_ascii_case(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn strip_svg_preamble(mut text: &str) -> Option<&str> {
    text = text.trim_start_matches('\u{feff}').trim_start();

    loop {
        if starts_with_ignore_ascii_case(text, "<?xml") {
            let end = text.find("?>")?;
            text = text[end + 2..].trim_start();
        } else if text.starts_with("<!--") {
            let end = text.find("-->")?;
            text = text[end + 3..].trim_start();
        } else if starts_with_ignore_ascii_case(text, "<!doctype") {
            let end = text.find('>')?;
            text = text[end + 1..].trim_start();
        } else {
            return Some(text);
        }
    }
}

fn is_svg_open_tag(text: &str) -> bool {
    if !starts_with_ignore_ascii_case(text, "<svg") {
        return false;
    }

    matches!(
        text.as_bytes().get(4),
        Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r')
    )
}

fn is_svg_content(content: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(content) else {
        return false;
    };

    strip_svg_preamble(text).is_some_and(is_svg_open_tag)
}

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

fn terminal_image_protocol_from_env(
    kitty_window_id: Option<&str>,
    term: Option<&str>,
    term_program: Option<&str>,
) -> Option<TerminalImageProtocol> {
    if kitty_window_id.is_some()
        || term.is_some_and(|term| contains_ignore_ascii_case(term, "kitty"))
        || term_program.is_some_and(|term_program| {
            ["WezTerm", "ghostty", "kitty"]
                .iter()
                .any(|supporter| contains_ignore_ascii_case(term_program, supporter))
        })
    {
        return Some(TerminalImageProtocol::Kitty);
    }

    if term.is_some_and(|term| contains_ignore_ascii_case(term, "sixel"))
        || term_program
            .is_some_and(|term_program| contains_ignore_ascii_case(term_program, "iterm"))
    {
        return Some(TerminalImageProtocol::Sixel);
    }

    None
}

fn terminal_image_protocol() -> Option<TerminalImageProtocol> {
    let kitty_window_id = env::var("KITTY_WINDOW_ID").ok();
    let term = env::var("TERM").ok();
    let term_program = env::var("TERM_PROGRAM").ok();

    terminal_image_protocol_from_env(
        kitty_window_id.as_deref(),
        term.as_deref(),
        term_program.as_deref(),
    )
}

fn tiff_image(image_data: &[u8]) -> io::Result<image::DynamicImage> {
    image::load_from_memory_with_format(image_data, ImageFormat::Tiff)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn png_from_tiff(image_data: &[u8]) -> io::Result<Vec<u8>> {
    let img = tiff_image(image_data)?;
    let mut png_data = Cursor::new(Vec::new());
    img.write_to(&mut png_data, ImageFormat::Png)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(png_data.into_inner())
}

fn quantize_sixel_component(value: u8) -> u8 {
    ((value as u16 * (SIXEL_COLOR_LEVELS as u16 - 1) + 127) / 255) as u8
}

fn sixel_palette_index(red: u8, green: u8, blue: u8) -> u8 {
    let red = quantize_sixel_component(red);
    let green = quantize_sixel_component(green);
    let blue = quantize_sixel_component(blue);

    red * 36 + green * 6 + blue
}

fn sixel_palette_color(index: usize) -> (usize, usize, usize) {
    let red = index / 36;
    let green = (index / 6) % 6;
    let blue = index % 6;

    (red * 20, green * 20, blue * 20)
}

fn push_sixel_mask_run(output: &mut String, mask: u8, count: usize) {
    let sixel_char = (mask + 63) as char;

    if count > 3 {
        write!(output, "!{}{}", count, sixel_char).unwrap();
    } else {
        for _ in 0..count {
            output.push(sixel_char);
        }
    }
}

fn push_sixel_masks(output: &mut String, masks: &[u8]) {
    if masks.is_empty() {
        return;
    }

    let mut current = masks[0];
    let mut count = 1;

    for &mask in &masks[1..] {
        if mask == current {
            count += 1;
        } else {
            push_sixel_mask_run(output, current, count);
            current = mask;
            count = 1;
        }
    }

    push_sixel_mask_run(output, current, count);
}

fn encode_sixel_image(rgba: &image::RgbaImage) -> String {
    let (width, height) = rgba.dimensions();
    let width = width as usize;
    let height = height as usize;
    let pixels = rgba.as_raw();
    let mut indexed_pixels = vec![SIXEL_TRANSPARENT; width * height];
    let mut used_colors = [false; SIXEL_PALETTE_SIZE];

    for y in 0..height {
        for x in 0..width {
            let pixel_index = (y * width + x) * 4;
            let alpha = pixels[pixel_index + 3];

            if alpha < SIXEL_ALPHA_THRESHOLD {
                continue;
            }

            let palette_index = sixel_palette_index(
                pixels[pixel_index],
                pixels[pixel_index + 1],
                pixels[pixel_index + 2],
            );
            indexed_pixels[y * width + x] = palette_index;
            used_colors[palette_index as usize] = true;
        }
    }

    let mut output = String::new();
    write!(output, "\x1bPq\"1;1;{};{}", width, height).unwrap();

    for (index, is_used) in used_colors.iter().enumerate() {
        if *is_used {
            let (red, green, blue) = sixel_palette_color(index);
            write!(output, "#{};2;{};{};{}", index, red, green, blue).unwrap();
        }
    }

    for band_start in (0..height).step_by(6) {
        let mut band_masks = vec![vec![0u8; width]; SIXEL_PALETTE_SIZE];
        let mut band_colors = [false; SIXEL_PALETTE_SIZE];

        for y_offset in 0..6 {
            let y = band_start + y_offset;
            if y >= height {
                break;
            }

            for x in 0..width {
                let palette_index = indexed_pixels[y * width + x];
                if palette_index == SIXEL_TRANSPARENT {
                    continue;
                }

                band_masks[palette_index as usize][x] |= 1 << y_offset;
                band_colors[palette_index as usize] = true;
            }
        }

        let mut wrote_color = false;
        for (index, is_used) in band_colors.iter().enumerate() {
            if !*is_used {
                continue;
            }

            if wrote_color {
                output.push('$');
            }

            write!(output, "#{}", index).unwrap();
            push_sixel_masks(&mut output, &band_masks[index]);
            wrote_color = true;
        }

        output.push('-');
    }

    output.push_str("\x1b\\");
    output
}

fn write_kitty_graphics(image_data: &[u8]) -> io::Result<()> {
    let png_bytes = png_from_tiff(image_data)?;
    let encoded = BASE64.encode(&png_bytes);

    let mut stdout = io::stdout();

    // Kitty graphics protocol:
    // ESC_G<control data>;<payload>ESC\
    // a=T: transmit and display
    // f=100: PNG format
    // For large payloads, we chunk with m=1 (more data) and m=0 (last chunk)
    const CHUNK_SIZE: usize = 4096;
    let chunks: Vec<&str> = encoded
        .as_bytes()
        .chunks(CHUNK_SIZE)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect();

    for (i, chunk) in chunks.iter().enumerate() {
        let is_last = i == chunks.len() - 1;
        let is_first = i == 0;

        if is_first && is_last {
            // Single chunk - no need for m parameter
            write!(stdout, "\x1b_Ga=T,f=100;{}\x1b\\", chunk)?;
        } else if is_first {
            // First chunk of multi-chunk transmission
            write!(stdout, "\x1b_Ga=T,f=100,m=1;{}\x1b\\", chunk)?;
        } else if is_last {
            // Last chunk
            write!(stdout, "\x1b_Gm=0;{}\x1b\\", chunk)?;
        } else {
            // Middle chunk
            write!(stdout, "\x1b_Gm=1;{}\x1b\\", chunk)?;
        }
    }

    // Add newline after image
    writeln!(stdout)?;
    stdout.flush()?;

    Ok(())
}

fn write_sixel_graphics(image_data: &[u8]) -> io::Result<()> {
    let rgba = tiff_image(image_data)?.to_rgba8();
    let sixel = encode_sixel_image(&rgba);

    let mut stdout = io::stdout();
    write!(stdout, "{}", sixel)?;
    writeln!(stdout)?;
    stdout.flush()?;

    Ok(())
}

fn pbcopy() -> Result<(), Box<dyn Error>> {
    let mut content = Vec::new();
    io::stdin().read_to_end(&mut content)?;

    if is_svg_content(&content) {
        return set_pasteboard_svg(content);
    }

    if let Some(tiff_data) = tiff_from_image_data(&content) {
        return set_pasteboard_tiff(tiff_data);
    }

    let text = String::from_utf8(content).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "stdin is neither supported image data nor UTF-8 text",
        )
    })?;

    set_pasteboard_text(&text)
}

fn pbpaste() -> Result<(), Box<dyn Error>> {
    match get_clipboard_content()? {
        ClipboardContent::Text(text) => {
            io::stdout().write_all(&text)?;
        }
        ClipboardContent::Svg(svg) => {
            io::stdout().write_all(&svg)?;
        }
        ClipboardContent::Image(image_data) => match stdout_output_device() {
            "file" => {
                if let Ok(extension) = get_stdout_filename_extension() {
                    if let Some(transformed) = transform_content(&extension, &image_data) {
                        io::stdout().write_all(&transformed)?;
                    }
                }
            }
            "terminal" => match terminal_image_protocol() {
                Some(TerminalImageProtocol::Kitty) => write_kitty_graphics(&image_data)?,
                Some(TerminalImageProtocol::Sixel) => write_sixel_graphics(&image_data)?,
                None => {
                    eprintln!("Terminal does not support Kitty graphics or Sixel output.");
                    eprintln!("Supported terminals: Kitty, WezTerm, Ghostty, iTerm2");
                }
            },
            _ => {
                eprintln!("Unsupported clipboard content");
            }
        },
        ClipboardContent::Unknown => {
            eprintln!("Unsupported clipboard content");
        }
    }

    Ok(())
}

fn run() -> Result<(), Box<dyn Error>> {
    match action_for_stdin(stdin_is_terminal()) {
        ClipboardAction::Copy => pbcopy(),
        ClipboardAction::Paste => pbpaste(),
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("pbi: {}", err);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        action_for_stdin, encode_sixel_image, is_svg_content, terminal_image_protocol_from_env,
        ClipboardAction, TerminalImageProtocol,
    };
    use image::{Rgba, RgbaImage};

    #[test]
    fn copy_when_stdin_is_not_terminal() {
        assert_eq!(action_for_stdin(false), ClipboardAction::Copy);
    }

    #[test]
    fn paste_when_stdin_is_terminal() {
        assert_eq!(action_for_stdin(true), ClipboardAction::Paste);
    }

    #[test]
    fn uses_kitty_when_kitty_window_is_present() {
        assert_eq!(
            terminal_image_protocol_from_env(Some("1"), Some("xterm-256color"), Some("iTerm.app")),
            Some(TerminalImageProtocol::Kitty)
        );
    }

    #[test]
    fn uses_kitty_for_known_kitty_protocol_terminals() {
        assert_eq!(
            terminal_image_protocol_from_env(None, None, Some("ghostty")),
            Some(TerminalImageProtocol::Kitty)
        );
    }

    #[test]
    fn uses_sixel_for_iterm() {
        assert_eq!(
            terminal_image_protocol_from_env(None, Some("xterm-256color"), Some("iTerm.app")),
            Some(TerminalImageProtocol::Sixel)
        );
    }

    #[test]
    fn uses_sixel_for_sixel_term() {
        assert_eq!(
            terminal_image_protocol_from_env(None, Some("xterm-sixel"), None),
            Some(TerminalImageProtocol::Sixel)
        );
    }

    #[test]
    fn ignores_unknown_terminal_protocols() {
        assert_eq!(
            terminal_image_protocol_from_env(None, Some("xterm-256color"), Some("Apple_Terminal")),
            None
        );
    }

    #[test]
    fn encodes_single_red_pixel_as_sixel() {
        let image = RgbaImage::from_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let sixel = encode_sixel_image(&image);

        assert!(sixel.starts_with("\x1bPq\"1;1;1;1#180;2;100;0;0"));
        assert!(sixel.contains("#180@"));
        assert!(sixel.ends_with("-\x1b\\"));
    }

    #[test]
    fn skips_transparent_pixels_in_sixel_output() {
        let image = RgbaImage::from_pixel(1, 1, Rgba([255, 0, 0, 0]));
        let sixel = encode_sixel_image(&image);

        assert!(!sixel.contains(";2;"));
        assert!(!sixel.contains("#180"));
    }

    #[test]
    fn compresses_repeated_sixel_masks() {
        let image = RgbaImage::from_pixel(4, 1, Rgba([255, 0, 0, 255]));
        let sixel = encode_sixel_image(&image);

        assert!(sixel.contains("#180!4@"));
    }

    #[test]
    fn detects_plain_svg_document() {
        assert!(is_svg_content(
            br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#
        ));
    }

    #[test]
    fn detects_svg_with_xml_preamble() {
        assert!(is_svg_content(
            br#"<?xml version="1.0" encoding="UTF-8"?>
            <!-- generated -->
            <!DOCTYPE svg>
            <svg viewBox="0 0 10 10"></svg>"#
        ));
    }

    #[test]
    fn rejects_text_that_mentions_svg() {
        assert!(!is_svg_content(b"This sentence talks about <svg> tags."));
    }

    #[test]
    fn rejects_non_utf8_data_as_svg() {
        assert!(!is_svg_content(&[0xff, 0xd8, 0xff, 0x00]));
    }
}
