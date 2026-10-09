# Security policy

## What to report privately

procquarium is a process monitor, and two of its features act on the processes
it observes:

- `--kill` sends `SIGTERM` to a process you select, after a confirmation. It
  re-checks the process's start time just before signalling so a reused pid is
  never signalled, but a bug here could terminate the wrong program.
- `--feed` lowers the nice value of processes you own, and restores it on exit.

Report privately anything that could make `--kill` or `--feed` touch a process
they should not, or any other way the program can be made to act outside the
tank.

Recordings (`--record`) contain process names, pids and usernames from the
machine that made them. Treat a recording you are asked to share as private
data and skim it before attaching it to an issue.

## How to report

Use GitHub's private vulnerability reporting: open the **Security** tab of
[ImanolGo/procquarium](https://github.com/ImanolGo/procquarium/security) and
choose **Report a vulnerability**. If that is not available to you, open a
normal issue that says only that you have a security problem and asks for a
private channel; do not include the details or a reproduction in public.

Please include the version (`procquarium --version`), your platform, and as much
of a reproduction as you can share safely.

## Supported versions

Security fixes land on the latest release. There are no backports to older
versions.
