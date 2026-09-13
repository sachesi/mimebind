# Security

Please report a vulnerability privately rather than in a public issue: through
[a private advisory](https://github.com/sachesi/mimebind/security/advisories/new) on
GitHub, or by mail to sachesi <xsachesi@pm.me>. Say what you found, how to reproduce it and
which version you ran; a fix is worked out with you before anything is published.

Only the latest release gets fixes.

## What counts

Mimebind decides which program the desktop starts for a file or a link. It reads desktop
entries and the MIME database, and writes the user's `mimeapps.list` through GIO. A way to
make it write an association the user did not choose, or to write outside that file, is a
vulnerability.

An association that comes out wrong because an application declares types it cannot
handle is a bug; please file it as an ordinary issue.
