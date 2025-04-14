# Hyprpaper Unsplash Wallpaper

Bored of looking at the same wallpaper for weeks? 🤔 Look no further! 😁
This tools requests a random wallpaper from [*Unsplash*](https://unsplash.com/)
and sets it as the current wallpaper to [Hyprlands](https://hyprland.org/)
[*Hyprpaper*](https://wiki.hyprland.org/Hypr-Ecosystem/hyprpaper/).

## Installation

Use the rust package manager `cargo` to install *Hyprpaper Unsplash Wallpaper*.

1. [Install Rust](https://www.rust-lang.org/tools/install)
2. Fetch and build

```bash
cargo install --git https://github.com/qwertzui11/hyprpaper-unsplash
```

The resulting binary can be found inside of `~/.cargo/bin`. Afterwards,
you might want to add this to your `PATH` environment variable.

You need an *Unsplash API Key*. Go to [unsplash.com/developers](https://unsplash.com/developers)
and register. Then go to [*applications*](https://unsplash.com/oauth/applications)
and generate an `api-key` for this application.

Save that `api-key` to the file `~/.config/unsplash-key`

```bash
echo "$UNSPLASH_KEY" > ~/.config/unsplash-key
```

## Usage

To get a new wallpaper each time *Hyprland* starts add the following to your `~/.config/hypr/hyprland.conf`:

```hypr
exec-once = hyprpaper-unsplash
```

You can also get a random wallpaper for a search term.
For example to get a random wallpaper containing a banana run:

```bash
hyprpaper-unsplash --query banana
```

![banana wallpaper](https://images.unsplash.com/photo-1604247416031-b05e5451a7f9?q=80&w=800&auto=format&fit=crop)

After a reboot the new wallpaper is gone. To ensure the last downloaded image
is getting re-used add the following to `~/.config/hypr/hyprpaper.conf`:

```hypr
preload = ~/Pictures/hyprpaper-wallpaper.jpeg
wallpaper = ,~/Pictures/hyprpaper-wallpaper.jpeg
```

## Contributing

Pull requests are welcome. For major changes, please open an issue first
to discuss what you would like to change.

## License

Licensed under [MIT](LICENSE).
