# Tests and measurements

How to run the suites, and what is measured and what is not. Back to the [README](../README.md).

## Running the tests

    werkzeug/vor-dem-commit.sh          # everything, and stops on red
    werkzeug/bau.sh test --offline      # the Rust suite alone

The Rust suite runs in the pinned build image and prints its counts per file.
The repository fences run on the host, because they ask git. A case that
cannot put its question on this copy skips with its reason instead of passing.

## What is measured, and what is not

- The service runs as a static binary behind a TLS proxy on a host of the
  operator's; the app's sign-in, the document round trip and the changes
  socket are exercised there.
- The takeover end to end (a locked, playing device stops within the
  claim's cadence when the other starts) needs two devices and belongs to
  the device test.
- The fallback cadence when the changes socket is down (the app polls every
  thirty seconds until it reopens) is chosen, not measured; the measurement
  belongs to the same device test.

---

Other pages: [Setting it up by hand](setup.md) · [Running it in a container](container.md) · [The catalog index (optional)](catalog.md) · [Operating a running host](operation.md) · [Accounts and signing in](accounts.md) · [How it is built](design.md) · [Tests and measurements](testing.md)
