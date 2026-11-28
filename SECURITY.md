# Security Policy

## Supported Versions

The latest release receives security fixes. Older releases do not.

## Reporting a Vulnerability

Report a vulnerability privately through GitHub: open the repository's Security
tab and choose "Report a vulnerability", or go straight to
<https://github.com/yalongwastaken/git-koi/security/advisories/new>. Do not open
a public issue for a vulnerability.

Include the command or workflow that triggers the problem and what an attacker
gains from it. You will hear back within seven days, and the fix ships in a
release with credit to you unless you ask otherwise.

## Scope

`git-koi` reads only an account's public data, through the GitHub CLI and the
token the caller supplies, and writes only the files the caller names. The
Action runs inside the calling workflow with that workflow's permissions. A
report that shows it reading, writing, or sending anything beyond that is in
scope.

Whatever token a workflow passes the Action, the Action's process holds it, so
the token bounds what a compromised release could do. The default, the
workflow's built-in token, reaches only the calling repository, for one job,
with the workflow's permissions. A personal access token would hand over
everything it can reach, so keep the default.
