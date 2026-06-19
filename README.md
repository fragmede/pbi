# pbi

<p align="center">
  <img src="assets/pbi-icon.svg" alt="pbi clipboard icon" width="156">
</p>

## What is this?
pbi is a command line utility for the mac that copies stdin to the pasteboard
when stdin is piped or redirected. When stdin is a terminal, it pastes the
current pasteboard content.

### why is this better?
`pbcopy` and pbpaste` already exist, but to their detriment, they're separate
binaries.  We can just have one command because the process can detect if it's
being used as input or output.

Oh, it's also image aware. 

The macOS pasteboard has type information, so that you can paste formatting as
well as the text. Or you can copy images. With pbpaste though, you can't use
pbpaste to dump the image on your pasteboard to where you are in the terminal,
because `pbpaste` (and `pbcopy`) just silently fail if they don't find text.

So you can't right click, copy image, and then pbpaste that to a file in the terminal.

With pbi, you can.

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

There's also the other direction, so you can

    pbi < nyan.png

and then ⌘+N into preview.app will get you the image in preview.

Also you can do 

    cat nyan.png | pbi

    (`pbi < nyan.png` if you must)

If you use a modern terminal that has Kitty graphics protocol or sixel support,
with an image on your paste board you can also do:

    pbi

and the image will display directly in your terminal.

If you override `TERM` because of TERMCAP distribution delay, and terminal
image auto-detection fails, force the protocol explicitly:

    export PBI_IMAGE_PROTOCOL=sixel

# Pasteboard vs clibpard
## Fun fact!
Apple’s APIs call it a pasteboard (NSPasteboard, UIPasteboard) rather than a
clipboard. The term comes from old publishing workflows where designers
literally cut out text and images and pasted them onto layout boards before
printing. Copy/paste wasn’t originally a computer metaphor, it came from the
world of publishing.
