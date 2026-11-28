# Accounts and signing in

Laying down accounts, and how a device signs in since contract version 6. Back to the [README](../README.md).

## Accounts

Every path but `/v1/health`, `/v2/key`, `/v1/session` and the three sign-in
paths needs the header `Authorization: Bearer` followed by the token, and the account the request acts on comes
from the store's answer to that token, never from anything the caller wrote.

    maer-sync account <owner>            # password on stdin, never in argv
    maer-sync account-remove <owner>     # the account and its documents leave,
                                         # after a backup that rehearsed its restore
    maer-sync reset <owner>              # a one-time password that only changes the password
    maer-sync devices <owner>            # how each device of an account signed in last

### Signing in (version 6)

The exact wire stands in `CONTRACT.md`; this is what an operator needs to
know about it.

- **Three paths, only inside a session.** `/v1/salt` tells a device the salt
  and the form of an account, `/v1/token` trades a proof for a token,
  `/v1/password` changes the password. A plain request gets `426`.
- **The service holds a proof, stretched once more with scrypt**, never a
  password. A thief of the store can still guess passwords, at the cost of
  both stretchings per guess; the running service learns nothing it could open
  a sealed document with. Tokens are stored as digests, so a leaked store
  hands out no working credential.
- **A wrong proof and an owner that is not here get the same answer**, and
  `/v1/salt` answers every name with a salt of the same shape, so the paths
  cannot be used to find out who has an account.
- **A password change withdraws every other token of the owner**, and a
  device holding one is told why (`X-Maer-Refusal: password-changed`).
- **Ten failed attempts per owner name and address in fifteen minutes**, and
  the next one is `429` with `Retry-After`. See step 7 for the address.
- **`reset <owner>` makes a one-time password and shows it once.** It signs
  nobody in (`428`); it only lets a device set a new password, and every
  token of the account is withdrawn. The operator knows it, which is exactly
  why it can never become the key of the feed credentials.

The token rides in a header, never in the address: a query string is copied
into every access log and proxy record between the device and the service.
The WebSocket handshake takes headers too: a native client sets them, and
it is only browsers that cannot, which is why so many services end up
putting tokens in URLs.

---

Other pages: [Setting it up by hand](setup.md) · [Running it in a container](container.md) · [The catalog index (optional)](catalog.md) · [Operating a running host](operation.md) · [Accounts and signing in](accounts.md) · [How it is built](design.md) · [Tests and measurements](testing.md)
