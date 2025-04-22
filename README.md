# Rust fzf Clone

A simple clone of `fzf` written in Rust. 

# Why should I use this? 

You probably shouldn't.  Use proper `fzf`.  I wrote this in an afternoon to play with TUI stuff in Rust, play with fuzzy search algorithms, and play with different ways of lowering memory. 

`fzf` is probably less buggy and probably faster.  A lot more people have worked on it, and writing something in Rust doesn't magically make it fast.  

THAT SAID, I've been using it in Sway as my primary program launcher, and it seems to work fine, comparable to `fzf`, so take that for what it is. 


# How to build? 

If you have `nix`, you can simply run `nix build` to get a `musl` linked, optimized executable.  If you don't have Nix, you can simply do `cargo build`. 
