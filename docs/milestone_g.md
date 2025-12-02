Alright, let’s nail this one properly.

Here’s **Milestone G — serialsh mode** reworked as a clean, crate-driven, 100% Rust design **without code**, but concrete enough to implement straight away.

---

## 📌 Milestone G — CLI Integration Mode (`--serialsh`)

### High-level Goal

When invoked with a special flag, the program behaves like a **remote pseudo-shell**:

* Reads commands from the user line-by-line.
* Sends them over the **Milestone A command tunnel**.
* Streams stdout/stderr back in near real-time.
* Exits with the same exit code as the last remote command (or a clear, documented policy).

This is **pure text, non-PTY**, no TTY/termcap magic, and fully optional.

---

## 🎯 Behaviour & UX

### Invocation

* Default behaviour (`--run` or no flags): existing LCD / service mode, unchanged.
* New mode: `--serialsh` (plus optional flags: device, baud, maybe timeout).

Process stays in the **foreground**, like a shell or `ssh` session.

### User Experience

* Prompt text is simple (`> ` or similar), configurable later.
* User types a line → it’s sent as a single Milestone A command request over the tunnel.
* Output appears as soon as chunks arrive:

  * stdout chunks rendered as-is
  * stderr chunks optionally prefixed or distinguished (configurable later, but plain text by default)
* When the remote process exits:

  * The prompt returns for the next command.
  * The effective exit code is tracked for tests and script integration.

### Compatibility Constraints

* `--serialsh` **must not** become the default mode.
* Systemd or boot service units continue to use the existing mode by default.
* `--serialsh` is **opt-in** and must not be enabled silently by configs.

---

## 🧱 Architectural Integration

### Components Involved

* `src/cli.rs`

  * Extends argument parsing to understand `--serialsh`.
  * Handles CLI-only settings (history toggle, device, baud, timeouts).

* `src/app/mod.rs`

  * Adds a **“CLI front-end”** mode that:

    * Initializes serial connection.
    * Runs the Milestone A tunnel + heartbeat logic.
    * Wraps an interactive loop around the command tunnel.

* `src/app/connection.rs` / `render_loop.rs`

  * Expose a “command tunnel client API” that:

    * Sends a single command request.
    * Returns a stream of stdout/stderr chunks + final exit status.
  * In serialsh mode, LCD stuff can be either disabled or minimally engaged; priority is command tunnel.

* `docs/README.md`

  * New section describing serialsh usage, caveats, and examples.

---

## 🧩 CLI Parsing & Options

### Parser Behaviour (in `src/cli.rs`)

Extend existing argument parsing (currently `clap`-style custom):

* Recognise `--serialsh` as a **mode** flag.
* Additional flags for this mode, for example:

  * `--device` or `--port` (path to serial device, optional if default exists).
  * `--baud` (fallback to sane default).
  * `--history` toggle (on/off).
  * `--timeout` for per-command or idle timeout, if desired.

The parser must clearly differentiate between:

* “Run as service / LCD daemon”
* “Run as serialsh interactive CLI”

No overlapping or ambiguous flags.

---

## 🔄 Command Loop Model

### Core Flow

1. Initialize:

   * Open serial port using your standard serial backend.
   * Run handshakes / negotiation (Milestone B) to ensure we’re in the **command client** role.
   * Initialize the command tunnel interfaces from Milestone A (send command, receive streams).

2. Enter interactive loop:

   * Read a single line of input from stdin.
   * If EOF (Ctrl+D) or “exit”/empty line logic: break loop and terminate properly.
   * Send the line as a tunneled command request.
   * While waiting for response:

     * Stream stdout/stderr back as soon as chunks arrive.
     * Watch for remote exit message and final status.
   * Return to input prompt.

3. Exit:

   * Close serial gracefully.
   * Exit process with appropriate exit code (policy defined below).

### Line Input / History

* Primary option: **simple line-based input** using standard input/output.
* Optional enhancement: if allowed, integrate `rustyline` or similar:

  * Command history.
  * Basic line-editing (arrow keys, etc.).
  * History file location:

    * Under RAM disk (`/run/serial_lcd_cache/serialsh_history`) to avoid writing outside configured constraints, or configurable.
* History must be **optional** and disabled in constrained environments.

---

## 🧨 Signals & Ctrl+C Handling

### Using the `ctrlc` crate

* global handler that:

  * In `serialsh` mode: does **not** immediately terminate the whole program.
  * Instead:

    * Sends a **“terminate current command”** message through the command tunnel (e.g. a control packet defined in Milestone A/B world).
    * Marks the current command as “interrupted” locally.
  * If repeated or if there is no active command:

    * On second Ctrl+C, exits the CLI process (documented behaviour).

### Requirements

* Signal handler must be cheap (no heavy work in the handler).
* Proper coordination with the async runtime / event loop:

  * Typically by notifying a dedicated task via an atomic flag or channel.

---

## 🧾 Output Formatting Rules

* **Plain text only by default.**

  * No hard dependency on ANSI/colour escapes.
  * Just pass through remote stdout/stderr chunks verbatim.
* Basic separation logic:

  * Optionally prefix stderr lines with a token (e.g. “stderr: ”) for debugging, behind a flag.
  * By default, simply merge into the same output stream, like a normal terminal.
* Prompt rules:

  * Simple, consistent prompt string.
  * No fancy dynamic path detection or $PS1-style environment.
  * Keep it robust under incomplete or garbled outputs.

---

## 📑 Exit Code Semantics

Decide and document a clear policy for process exit:

* If no commands were successfully run:

  * Exit code `0` (successful session) **or** a distinct “no command” code (but keep it simple initially).
* If one or more commands were run:

  * The **exit code of the last command** becomes the process exit status.
* If the remote side crashes, disconnects, or times out:

  * Non-zero exit code (a fixed value, e.g. 255).
  * Document that as “connection/transport failure” code.

This matters for scripts that wrap `serialsh`.

---

## 🧪 Testing Strategy

Integration tests (no actual code, just behaviour design):

* **Happy path**:

  * Fake serial backend returns simple command output (`echo hi` → “hi”).
  * Test that:

    * Input line triggers one remote request.
    * Output is printed in correct order.
    * The final exit code matches remote.

* **Error path**:

  * Remote returns non-zero exit.
  * CLI must propagate that exit status.

* **Transport failure**:

  * Fake backend simulates disconnection mid-command.
  * CLI:

    * Prints an error message.
    * Exits with “connection failure” code.

* **Ctrl+C handling**:

  * Simulate an interrupt during a long-running command.
  * Ensure a “terminate command” control flow is triggered, and CLI returns to prompt.
  * Repeat to ensure second interrupt cleanly exits process.

* **Legacy / Fallback**:

  * If the remote side does not support the command tunnel (Milestone A not available):

    * `serialsh` should fail early with a clear, user-facing error (e.g. “remote does not support command tunnel”), not fall back silently to random LCD-only behaviour.

All of this is testable using:

* A fake serial endpoint.
* Event simulation for stdin, signals, and remote frames.

---

## 🔐 Safety & Constraints

* No PTYs, no TTY manipulations: avoids a whole class of complexity.
* No configuration or hidden behaviour that flips into serialsh automatically.
* No writes outside:

  * `/run/serial_lcd_cache` (for temporary files/history) and
  * explicit user paths/config paths you already defined (if at all).
* serialsh uses **only** the command tunnel and control-plane we’ve already specified in earlier milestones.

---

## ✅ What This Milestone Delivers

When this is done, you get:

* A **usable, scriptable CLI** that turns LifelineTTY into a pseudo-remote shell over UART.
* Clean interaction with the command tunnel (Milestone A), negotiation (Milestone B), and file transfer / future features.
* Well-structured, crate-backed design with signal handling and tests.
* No regression to existing modes or systemd units.

If you want, next step we can design the **exact command tunnel “API surface”** that serialsh will rely on (conceptually: “send command, subscribe to output, await exit”) so you have a clear internal boundary between CLI and core tunnel logic.
