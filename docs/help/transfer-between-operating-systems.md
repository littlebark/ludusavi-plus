# Transfer between operating systems
Although Ludusavi itself runs on Windows, Linux, and Mac,
it does not automatically support backing up on one OS and restoring on another.

This is a complex problem to solve because
games do not necessarily store data in the same way on each OS.
Ludusavi only knows where each game stores its data on a given OS,
but does not know which save locations correspond to each other,
or even if any of them do correspond.
Some games store data in completely different and incompatible ways on different OSes.

In simple cases, you may be able to configure [redirects](/docs/help/redirects.md)
to translate between specific Windows and Linux paths,
but this would generally require multiple redirects tailored to each game.
In more complex cases, this is not practical or feasible.

A subset of cross-OS transfer is under consideration for Windows and Wine prefixes,
but there is no timeline for this.
You can follow this ticket for any future updates:
https://github.com/mtkennerly/ludusavi/issues/194

<!--
## Windows and Wine
Ludusavi supports an experimental feature to translate between Windows paths and Wine prefixes.
This currently only supports files, not registry values.

As this is an experimental feature, you must enable it via the [config file](/docs/help/configuration-file.md),
by setting `scan.redirectWine: true`.

A game must have a "preferred" Wine prefix.
For now, that means it must have a custom game entry,
and the preferred prefix is simply the first one defined by the custom game entry.

There are some exclusions where the automatic Wine redirects will not apply:

* No preferred Wine prefix
* Wine prefix does not already exist
* Wine prefix has multiple potential user folders
* Windows path is on a complex UNC drive instead of a simple drive letter
-->
