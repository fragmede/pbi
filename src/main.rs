use std::env;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::ptr;
use std::slice;
use std::str;

use image::ImageFormat;
use objc::{msg_send, sel, sel_impl};
use objc::runtime::{Object, BOOL, YES};
use objc::runtime::{Class, Sel};
use objc_foundation::{INSObject, NSString, NSArray, NSData};
use objc_id::Id;
use std::ffi::CStr;

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
    } else if statbuf.st_mode & libc::S_IFMT == libc::S_IFCHR {
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

fn get_clipboard_content() -> (&'static str, Option<Vec<u8>>) {
    unsafe {
        let pb: *mut Object = msg_send![Class::get("NSPasteboard").unwrap(), generalPasteboard];
        let types: Id<NSArray<NSString>> = msg_send![pb, types];

        let nsstring_type: Id<NSString> = Id::from_ptr(msg_send![Class::get("NSString").unwrap(), stringWithUTF8String: "NSStringPboardType"]);
        let nstiff_type: Id<NSString> = Id::from_ptr(msg_send![Class::get("NSString").unwrap(), stringWithUTF8String: "NSTIFFPboardType"]);

        if types.contains_object(&nsstring_type) {
            let content: Id<NSString> = msg_send![pb, stringForType: nsstring_type];
            return ("text", Some(content.as_bytes().to_vec()));
        } else if types.contains_object(&nstiff_type) {
            let data: Id<NSData> = msg_send![pb, dataForType: nstiff_type];
            return ("image", Some(data.bytes().to_vec()));
        }
    }
    ("unknown", None)
}

fn transform_content(extension: &str, content: &[u8]) -> Option<Vec<u8>> {
    let formats = [
        (vec!["png"], ImageFormat::Png),
        (vec!["jpg", "jpeg"], ImageFormat::Jpeg),
        (vec!["gif"], ImageFormat::Gif),
        (vec!["bmp"], ImageFormat::Bmp),
        (vec!["heif"], ImageFormat::Heif),
        (vec!["webp"], ImageFormat::WebP),
    ];

    if !extension.is_empty() {
        for (exts, format) in formats.iter() {
            if exts.contains(&extension.to_lowercase().as_str()) {
                let img = image::load_from_memory_with_format(content, ImageFormat::Tiff).unwrap();
                let mut output = Vec::new();
                img.write_to(&mut output, *format).unwrap();
                return Some(output);
            }
        }
    }

    None
}

fn term_supports_sixel() -> bool {
    match env::var("TERM_PROGRAM") {
        Ok(val) if val == "Apple_Terminal" => return false,
        Ok(val) if val == "iTerm.app" => return true,
        _ => {}
    }

    if let Ok(output) = Command::new("tput").arg("setab").arg("0").output() {
        if str::from_utf8(&output.stdout).unwrap_or("").contains("sixel")
            || str::from_utf8(&output.stderr).unwrap_or("").contains("sixel")
        {
            return true;
        }
    }

    false
}

fn pbpaste() {
    let (content_type, content) = get_clipboard_content();

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
                        if term_supports_sixel() {
                            let mut child = Command::new("magick")
                                .arg("-")
                                .arg("sixel:-")
                                .stdin(io::stdio::piped())
                                .stdout(io::stdio::inherit())
                                .stderr(io::stdio::inherit())
                                .spawn()
                                .unwrap();

                            if let Some(stdin) = child.stdin.as_mut() {
                                stdin.write_all(&image_data).unwrap();
                            }

                            child.wait().unwrap();
                        } else {
                            println!("Cowardly not printing image data to stdout.");
                        }
                    }
                    _ => {
                        println!("Unsupported clipboard content");
                    }
                }
            }
        }
        _ => {
            println!("Unsupported clipboard content");
        }
    }
}

fn main() {
    pbpaste();
}
