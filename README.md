# pbi

Image-aware version of `pbcopy` and `pbpaste` for macOS.

Pbcopy/paste gets/sets information on the clipboard from the terminal.
They're pretty handy! They don't handle images though. So you can't right
click, copy image, and then paste that contents in the terminal.

With pbi, you can.

While we're at it, pbcopy and pbpaste can be combined into one utility that
detects read or write.

So with this you can do:

    pbi > nyan.jpg

or

    pbi > nyan.png

We live in the future and ghostty has kitty support, so you can also do

    pbi

and it'll display the image in a supported TERM.


`pbi` copies stdin to the pasteboard when stdin is piped or redirected. When
stdin is a terminal, it pastes the current pasteboard content.
