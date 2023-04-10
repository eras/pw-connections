Copyright 2022 Erkki Seppälä <erkki.seppala@vincit.fi>

Licensed under the [`MIT license`](LICENSE.MIT).

# pw-connections

`pw-connections` is a simple tool to setup and maintain Pipewire node
connections even if devices disappear and come back.

# Installing

Grab a binary built on Linux x86/64 Ubuntu from Releases, `chmod +x`
it and move it to a directory of your choice.

You can also compile it yourself. To compile it you first need have a
the Rust compiler. If your operating system doesn't come with the Rust
compiler, or the compiler is too old, then the easiest way to install
one is to use [`rustup`](https://rustup.rs/).

Once the compiler (and `cargo`, the Rust package manager) is
installed, you can just run the command `cargo install --git
https://github.com/eras/pw-connections` to install the latest version to
`~/.cargo/bin`.

# Usage

For basic use you can use

```
pw-connections --config connections.yaml
```

And the connections.yaml might look some like:

```
links:
  - src: "Virtual Raw MIDI 4-1 4:(capture_0) VirMIDI 4-1"
    dst: "Novation SL MkIII 1:(playback_0) Novation SL MkIII MIDI 1"
  - src: "Virtual Raw MIDI 4-2 4:(capture_0) VirMIDI 4-2"
    dst: "Novation SL MkIII 1:(playback_1) Novation SL MkIII MIDI 2"
  - src: "Virtual Raw MIDI 4-3 4:(capture_0) VirMIDI 4-3"
    dst: "Novation SL MkIII 1:(playback_2) Novation SL MkIII MIDI 3"
  - src: "Novation SL MkIII 1:(capture_0) Novation SL MkIII MIDI 1"
    dst: "Virtual Raw MIDI 4-1 4:(playback_0) VirMIDI 4-1"
  - src: "Novation SL MkIII 1:(capture_1) Novation SL MkIII MIDI 2"
    dst: "Virtual Raw MIDI 4-2 4:(playback_0) VirMIDI 4-2"
  - src: "Novation SL MkIII 1:(capture_2) Novation SL MkIII MIDI 3"
    dst: "Virtual Raw MIDI 4-3 4:(playback_0) VirMIDI 4-3"
  - src: "Novation SL MkIII 1:(capture_0) Novation SL MkIII MIDI 1"
    dst: "ALC257 Analog:playback_FR"
```

You can get this kind of output with `pw-connections --dump` as a nice
starting point.

Recall that you can pretty much embed JSON inside YAML, so you should
be able to use JSON as configuration as well.

You can see the names using the excellent
[`jq`](https://stedolan.github.io/jq/) tool with e.g.

```
% pw-dump | jq '.[].info.props."port.alias"|select(.)'
"Midi Through:(capture_0) Midi Through Port-0"
"Midi Through:(playback_0) Midi Through Port-0"
"Novation SL MkIII 1:(capture_0) Novation SL MkIII MIDI 1"
"Novation SL MkIII 1:(capture_1) Novation SL MkIII MIDI 2"
"Novation SL MkIII 1:(capture_2) Novation SL MkIII MIDI 3"
"Novation SL MkIII 1:(playback_0) Novation SL MkIII MIDI 1"
"Novation SL MkIII 1:(playback_1) Novation SL MkIII MIDI 2"
"Novation SL MkIII 1:(playback_2) Novation SL MkIII MIDI 3"
"Novation SL MkIII 1:(playback_3) Novation SL MkIII MIDI 4"
"Novation SL MkIII 1:(playback_4) Novation SL MkIII MIDI 5"
"Virtual Raw MIDI 4-0 4:(capture_0) VirMIDI 4-0"
"Virtual Raw MIDI 4-0 4:(playback_0) VirMIDI 4-0"
"Virtual Raw MIDI 4-1 4:(capture_0) VirMIDI 4-1"
"Virtual Raw MIDI 4-1 4:(playback_0) VirMIDI 4-1"
"Virtual Raw MIDI 4-2 4:(capture_0) VirMIDI 4-2"
"Virtual Raw MIDI 4-2 4:(playback_0) VirMIDI 4-2"
"Virtual Raw MIDI 4-3 4:(capture_0) VirMIDI 4-3"
"Virtual Raw MIDI 4-3 4:(playback_0) VirMIDI 4-3"
"ALC257 Analog:monitor_FL"
"ALC257 Analog:monitor_FR"
"ALC257 Analog:playback_FL"
"ALC257 Analog:playback_FR"
```

Once running, `pw-connections` should be fire and forget. Connections
will be removed when you terminate the process. Consider `systemd`
user services to just keep it always running.
