NsWrap
===

A rust library that provide program interface of Linux container technology.

These technologies include system calls like `namespaces(7)` and `clone(2)`.
It can be use as a low-level library to configure and
execute program and closure inside linux containers.

The `Wrap` follows a similar builder pattern to std::process::Command.
In addition, `Wrap` contains methods to configure linux namespaces,
chroots, mount points, and more part specific to linux.

---

License
---

This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain one at <https://mozilla.org/MPL/2.0/>.

This software is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
