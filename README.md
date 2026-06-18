# pbi

<p align="center">
  <img src="assets/pbi-icon.svg" alt="pbi clipboard icon" width="156">
</p>

## What
An image-aware version of `pbcopy` and `pbpaste` for macOS.

Pbcopy/paste gets/sets information on the clipboard from the terminal.
They're pretty handy! They don't handle images though. So you can't right
click, copy image, and then paste that contents in the terminal.

With pbi, you can.

While we're at it, pbcopy and pbpaste can be combined into one utility that
detects read or write.

## Installing

Clone the repo, cargo build, copy the binary to somewhere in your $PATH

## Usage

Right click on an image, and then you can do:

    pbi > nyan.jpg

or

    pbi > nyan.png

Also you can do 

    cat nyan.png | pbi

    or 
    
    pbi < nyan.png
    
    if you prefer

We live in the future: terminals with Kitty graphics protocol support and
iTerm2 with Sixel support can also do plain

    pbi

and it'll display the image directly in the terminal.


`pbi` copies stdin to the pasteboard when stdin is piped or redirected. When
stdin is a terminal, it pastes the current pasteboard content.
