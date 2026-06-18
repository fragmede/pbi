# pbi

<p align="center">
  <img src="assets/pbi-icon.svg" alt="pbi clipboard icon" width="156">
</p>

## What is this?
`pbi` copies stdin to the pasteboard when stdin is piped or redirected. When
stdin is a terminal, it pastes the current pasteboard content.

### Advanced 
An image-aware version of `pbcopy` and `pbpaste` for macOS.

Pbcopy/paste gets/sets information on the clipboard from the terminal.
They're pretty handy! They don't handle images though. So you can't right
click, copy image, and then paste that contents in the terminal.

With pbi, you can.

While we're at it, pbcopy and pbpaste can be combined into one utility that
detects if you're reading or writing to it.

## Installing

From crates.io:

    cargo install pbi --locked

From a checkout:

    cargo install --path . --locked

## Usage

Show command help:

    pbi --help

Options:

- `--debug` prints clipboard and terminal diagnostics to stderr.
- `-h`, `--help` prints usage and exits without clipboard access.

Right click on an image in your web browser, copy image, and then your terminal, do:

    pbi > nyan.jpg

or

    pbi > nyan.png

Also you can do 

    cat nyan.png | pbi

    or 
    
    pbi < nyan.png
    
    if you prefer

If you use a modern terminal that has Kitty graphics protocol or sixel support,
with an image on your paste board you can do:

    pbi

and the image will display directly in your terminal.

If you override `TERM` and terminal image auto-detection fails, force the
protocol explicitly:

    export PBI_IMAGE_PROTOCOL=sixel
