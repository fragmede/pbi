use std::env;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::io::{self, Cursor, Read, Write};
use std::os::unix::io::AsRawFd;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::ImageFormat;
use libc::S_IFCHR;
use objc::runtime::{Class, Object, BOOL, YES};
use objc::{msg_send, sel, sel_impl};
use objc_foundation::{INSData, INSString, NSData, NSString};
use objc_id::Id;
use std::fmt;

const TEXT_PASTEBOARD_TYPES: [&str; 2] = ["public.utf8-plain-text", "NSStringPboardType"];
const TIFF_PASTEBOARD_TYPES: [&str; 2] = ["public.tiff", "NSTIFFPboardType"];

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
    Unknown,
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

    if let Some(text) = pasteboard_string_for_types(pb, &TEXT_PASTEBOARD_TYPES)? {
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

fn term_supports_kitty() -> bool {
    // Check for Kitty terminal
    if env::var("KITTY_WINDOW_ID").is_ok() {
        return true;
    }

    // Check TERM variable for kitty
    if let Ok(term) = env::var("TERM") {
        if term.contains("kitty") {
            return true;
        }
    }

    // Check TERM_PROGRAM for known Kitty graphics protocol supporters
    if let Ok(term_program) = env::var("TERM_PROGRAM") {
        // WezTerm, Ghostty, and Kitty support Kitty graphics protocol
        let kitty_supporters = ["WezTerm", "ghostty", "kitty"];
        for supporter in &kitty_supporters {
            if term_program
                .to_lowercase()
                .contains(&supporter.to_lowercase())
            {
                return true;
            }
        }
    }

    false
}

fn write_kitty_graphics(image_data: &[u8]) -> io::Result<()> {
    // Convert TIFF to PNG first
    let img = image::load_from_memory_with_format(image_data, ImageFormat::Tiff)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut png_data = Cursor::new(Vec::new());
    img.write_to(&mut png_data, ImageFormat::Png)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let png_bytes = png_data.into_inner();
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

fn pbcopy() -> Result<(), Box<dyn Error>> {
    let mut content = Vec::new();
    io::stdin().read_to_end(&mut content)?;

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
        ClipboardContent::Image(image_data) => match stdout_output_device() {
            "file" => {
                if let Ok(extension) = get_stdout_filename_extension() {
                    if let Some(transformed) = transform_content(&extension, &image_data) {
                        io::stdout().write_all(&transformed)?;
                    }
                }
            }
            "terminal" => {
                if term_supports_kitty() {
                    write_kitty_graphics(&image_data)?;
                } else {
                    eprintln!("Terminal does not support Kitty graphics protocol.");
                    eprintln!("Supported terminals: Kitty, WezTerm, Ghostty");
                }
            }
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
    use super::{action_for_stdin, ClipboardAction};

    #[test]
    fn copy_when_stdin_is_not_terminal() {
        assert_eq!(action_for_stdin(false), ClipboardAction::Copy);
    }

    #[test]
    fn paste_when_stdin_is_terminal() {
        assert_eq!(action_for_stdin(true), ClipboardAction::Paste);
    }
}
