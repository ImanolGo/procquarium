# procquarium

Your running processes, as fish.

procquarium is a terminal aquarium where every fish is a process on your machine. Big fish use a lot of memory, fast fish are burning CPU, and when a process exits its fish quietly floats to the surface. Leave it running in a spare pane or use it as a screensaver, and you'll start to recognise your machine's habits: the browser whale that never stops growing, the swarm of tiny shell fish that appear every time you run a build.

> **Status:** early days. The plan lives in [PLAN.md](PLAN.md) and things will move around a lot until 0.1.

```
  ~  ~   ~    ~      ~    ~   ~      ~    ~  ~
       °                         o
            ><(((°>    firefox
   ><>  ><>                            <°)))><
   zsh   zsh       ><>                 cargo
                   rustc       <><
     )                                  (
    (    ,      ><((°>         )         )
  ...)...(....,..............(...........(.....
```

## What the fish mean

| In the tank | On your machine |
| --- | --- |
| Fish size | Resident memory (log scale, so one huge process doesn't dwarf everything) |
| Swimming speed | CPU usage |
| Colour | Process name, so the same program always looks the same |
| A school following a bigger fish | Child processes following their parent |
| An egg hatching on the sand | A process that just started |
| A belly-up fish drifting upwards | A process that just exited |
| A grey fish with ✕ eyes | A zombie process |

## Install

You'll need a recent stable Rust toolchain.

```sh
cargo install --git https://github.com/<you>/procquarium
```

Once it's on crates.io, `cargo install procquarium` will do.

## Usage

```sh
procquarium                 # open the tank
procquarium --screensaver   # any key exits
procquarium --user $USER    # only your own processes
procquarium --max-fish 120  # crowded tank
```

Keys while it's running:

| Key | Does |
| --- | --- |
| `q` / `Esc` | Quit |
| `Space` | Pause the tank |
| `l` | Show or hide process names |
| `Tab` | Cycle through fish and show details (PID, CPU, memory) |
| `+` / `-` | More or fewer fish |

### As a real screensaver

Inside tmux you can have it start automatically after a few minutes of inactivity:

```tmux
set -g lock-after-time 300
set -g lock-command "procquarium --screensaver"
```

## Platforms

Linux and macOS are the main targets. Windows should work, but zombie fish won't show up there because Windows doesn't really have zombie processes.

## How it works

Once a second procquarium takes a snapshot of the process table with [`sysinfo`](https://crates.io/crates/sysinfo), compares it with the previous one, and turns the differences into events: births, deaths, things getting hungrier. The tank itself runs at around 30 frames per second and draws with [`ratatui`](https://ratatui.rs). Fish ease towards their new size and speed rather than jumping, so the picture stays calm even when your machine isn't.

procquarium tries hard to be a polite guest. It should stay under a couple of percent of one core, and yes, it shows up in its own tank.

## Contributing

Issues and pull requests are welcome. If you have an idea for a new creature (a crab for kernel threads? a jellyfish for containers?), open an issue first so we can talk it through.

## License

MIT, see [LICENSE](LICENSE).
