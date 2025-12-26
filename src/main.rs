use std::env;
use std::ffi::{CStr, CString};
use std::io::{self, Cursor, Write};
use std::os::unix::io::AsRawFd;
use std::error::Error;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::ImageFormat;
use libc::S_IFCHR;
use objc::{msg_send, sel, sel_impl};
use objc::runtime::{Class, Object};
use objc_foundation::{INSArray, INSData, NSArray, NSData, NSString};
use objc_id::Id;
use std::fmt;

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

impl Error for CocoaClassError {
    fn description(&self) -> &str {
        &self.details
    }
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

fn nsstring_to_str(nsstring: &NSString) -> Result<&str, Box<dyn Error>> {
    unsafe {
        let c_str: *const libc::c_char = msg_send![nsstring, UTF8String];
        if c_str.is_null() {
            return Err(Box::new(CocoaClassError::new("NSString", "UTF8String returned null")));
        }
        Ok(CStr::from_ptr(c_str).to_str()?)
    }
}

fn get_clipboard_content() -> Result<(&'static str, Option<Vec<u8>>), Box<dyn Error>> {
    unsafe {
        let pb: *mut Object = msg_send![Class::get("NSPasteboard").unwrap(), generalPasteboard];
        let types: Id<NSArray<NSString>> = msg_send![pb, types];

        let nsstring_type: Id<NSString> = Id::from_ptr(msg_send![Class::get("NSString").unwrap(), stringWithUTF8String: "NSStringPboardType\0"]);
        let nstiff_type: Id<NSString> = Id::from_ptr(msg_send![Class::get("NSString").unwrap(), stringWithUTF8String: "NSTIFFPboardType\0"]);
		let nsstring_str = nsstring_to_str(&*nsstring_type)?;
		let nstiff_str = nsstring_to_str(&*nstiff_type)?;
		println!("NSStringPboardType: '{}'", nsstring_str);
		println!("NSTIFFPboardType: '{}'", nstiff_str);

        let mut nsstring_found = false;
        let mut nstiff_found = false;

        for i in 0..types.count() {
            let obj: *mut Object = msg_send![types, objectAtIndex: i];
            let obj: Id<NSString> = Id::from_ptr(obj as *mut NSString);
            let obj_str = nsstring_to_str(&*obj)?;

            println!("Object at index '{}': '{:?}'", i, obj);
            println!("Object string: '{}'", obj_str);

            if obj_str == nsstring_str {
                nsstring_found = true;
            }
            else if obj_str == nstiff_str {
                nstiff_found = true;
            }
        }

        if nsstring_found {
            //println!("NSStringPboardType found");
            let content: Id<NSString> = msg_send![pb, stringForType: &*nsstring_type];
            let content_bytes: *const u8 = msg_send![content, UTF8String];
            let length = msg_send![content, lengthOfBytesUsingEncoding: 4 /* NSUTF8StringEncoding */];
            let bytes = std::slice::from_raw_parts(content_bytes, length);
            return Ok(("text", Some(bytes.to_vec())));
        } else if nstiff_found {
            println!("NSTIFFPboardType found");
            let data: Id<NSData> = msg_send![pb, dataForType: &*nstiff_type];
            let bytes_slice = std::slice::from_raw_parts(data.bytes().as_ptr(), data.bytes().len());
            return Ok(("image", Some(bytes_slice.to_vec())));
        }
        Ok(("unknown", None))
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
            if term_program.to_lowercase().contains(&supporter.to_lowercase()) {
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
    let chunks: Vec<&str> = encoded.as_bytes().chunks(CHUNK_SIZE)
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

fn pbpaste() {
    let result = get_clipboard_content();
    match result {
        Ok((content_type, content)) => {
            match content_type {
                "text" => {
                    if let Some(text) = content {
                        println!("{}", String::from_utf8_lossy(&text));
                    }
                }
                "image" => {
                    if let Some(image_data) = content {
                        match stdout_output_device() {
                            "file" => {
                                if let Ok(extension) = get_stdout_filename_extension() {
                                    if let Some(transformed) = transform_content(&extension, &image_data) {
                                        io::stdout().write_all(&transformed).unwrap();
                                    }
                                }
                            }
                            "terminal" => {
                                if term_supports_kitty() {
                                    if let Err(e) = write_kitty_graphics(&image_data) {
                                        eprintln!("Error displaying image: {}", e);
                                    }
                                } else {
                                    eprintln!("Terminal does not support Kitty graphics protocol.");
                                    eprintln!("Supported terminals: Kitty, WezTerm, Ghostty");
                                }
                            }
                            _ => {
                                println!("Unsupported clipboard content");
                            }
                        }
                    }
                }
                other => {
                    println!("Unsupported clipboard content {}", other);
                }
            }
        }
        Err(err) => {
            eprintln!("Error getting clipboard content: {}", err);
        }
    }
}

fn main() {
    pbpaste();
}
