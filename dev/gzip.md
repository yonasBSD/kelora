# Detect gzip by its magic bytes

## What to detect

Gzip starts with:

* 0x1F, 0x8B = magic
* 0x08 = compression method (deflate)

So `1F 8B 08` at the start is your green light. That’s stable and widely relied on.

## Why it’s reasonable for Kelora

You’ve sworn off “auto-magic” for content semantics. This isn’t that. Compression is transport. Lots of sane tools sniff gzip and just do the right thing. It reduces friction without violating the “logs are data, not text” mantra.

## Guardrails so it doesn’t get cute

* **Whitelist only gzip.** No auto-detect zoo. Don’t spiral into bz2/xz/zstd today. If you ever add more, make it explicit.
* **Stdin only or .gz filenames.**

  * If filename ends with `.gz`, decompress unconditionally.
  * For stdin, peek the first 3 bytes; if they match, wrap in gzip decoder. Otherwise pass through untouched.
* **No TAR spelunking.** If someone feeds `something.tar.gz`, you’ll output tar bytes after decompression and then parsing will choke. Good. Errors are visible. Don’t special-case archives.
* **Use a decoder that handles concatenated members.** Gzip streams can be “cat”-ed. Use a multi-member capable decoder so you don’t silently drop tail data.
* **Loud failures, quiet success.** On corruption, bail with a crisp error that names the decoder stage. When it works, don’t gloat.

## Implementation sketch (Rust, synchronous)

* Wrap your input `Read` in a small lookahead:

  1. Read up to 3 bytes into a buffer.
  2. Build a `Chain<Cursor<&[u8]>, R>` to put those bytes back in front.
  3. If they are `1F 8B 08`, wrap that chain in `flate2::read::MultiGzDecoder`.
  4. Else, use the chain directly as your source.

This avoids seeking and works for pipes.

```rust
use std::io::{self, Read, Cursor};
use std::io::Chain;
use flate2::read::MultiGzDecoder;

fn maybe_gzip<R: Read + 'static>(mut r: R) -> io::Result<Box<dyn Read>> {
    let mut head = [0u8; 3];
    let n = r.read(&mut head)?;
    let prefix = Cursor::new(&head[..n]);
    let chained: Chain<Cursor<&[u8]>, R> = prefix.chain(r);

    let is_gzip = n >= 3 && head[0] == 0x1F && head[1] == 0x8B && head[2] == 0x08;

    if is_gzip {
        Ok(Box::new(MultiGzDecoder::new(chained)))
    } else {
        Ok(Box::new(chained))
    }
}
```

Notes:

* Use `MultiGzDecoder`, not `GzDecoder`, so concatenated members don’t get dropped.
* If you care about performance, keep the outer reader buffered after this step.

## Flags and UX

Keep defaults minimal:

* Default: auto-detect gzip for stdin and `.gz` files. That’s it.
* Add `--no-decompress` if someone hates convenience out of principle.
* If you later add formats, make `--decompress=gzip[,zstd,…]` explicit, and keep the default as “gzip only.”

## Edge cases you’ll meet anyway

* **Random binary that starts `1F 8B 08`:** practically nonexistent for text logs. If someone handcrafts such a file, they’ve earned the confusion.
* **Truncated gzip:** decoder should error; surface that with a clear message.
* **Performance:** sniff cost is a 3-byte read. Congratulations, you survived.
