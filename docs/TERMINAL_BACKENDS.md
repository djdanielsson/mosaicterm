# Terminal backend probes

## Local and SSH persistence

A Linux probe with tmux 3.5a created a detached session on a private socket. The control client disconnected while a command was printing numbered lines. A second client attached to the same shell PID and working directory. The missed lines were available through `capture-pane`, and live control-mode `%output` messages carried OSC 133 start and finish markers as escaped bytes. Explicitly ending the session removed it from the server. [#132](https://github.com/djdanielsson/mosaicterm/issues/132)

A screen capture provides recent text. Restoring command blocks across a gap also requires a source for the original output and lifecycle events. The local probe did not restore MosaicTerm’s pane layout or test an SSH connection. The SSH design can keep session identity and ownership on the remote host while using the system SSH client as the transport. [#133](https://github.com/djdanielsson/mosaicterm/issues/133)

## Rex

Rex 0.1.0-preview.21, API revision 18, was tested on Linux x86-64 with a private Unix socket. Session listing, attachment, input, PTY resizing, and working-directory events worked. An event client disconnected during a running command and reattached to the original shell. The terminal still held the missed output, and the shell kept its PID and working directory. The event feed reported `terminal.child_exited` with code 9 when the shell exited. It did not report an exit event for commands that finished inside the shell. [#135](https://github.com/djdanielsson/mosaicterm/issues/135) · [Rex server API](https://www.superlogical.com/rex/docs/automate/server-api) · [Terminal events](https://www.superlogical.com/rex/docs/reference/events)

The restored VT screen omitted OSC 133 markers. The server describes a separate, ticketed `rex.data.terminal.v1` connection, but the probe did not verify whether it exposes ordered raw output with lifecycle markers or whether that stream can resume after a disconnect. The preview publishes Linux, macOS, and Windows builds. An optional connector would need version detection and a verified command event source before it could reconstruct MosaicTerm history. [Preview artifacts](https://blob.superlogical.com/api/releases/rex/preview)

## libghostty-vt

The upstream C API accepts an existing PTY stream and exposes terminal grid and render state. The documented examples cover live input, screen formatting, resizing, and selection. This makes it a candidate for the fullscreen overlay independently of session ownership and command history. A live MosaicTerm overlay, a resized editor session, and native packaging on Linux, macOS, and Windows still need verification. [#131](https://github.com/djdanielsson/mosaicterm/issues/131) · [Upstream C API](https://github.com/ghostty-org/ghostty/blob/main/include/ghostty/vt.h)
