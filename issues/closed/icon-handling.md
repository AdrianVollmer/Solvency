The icons are still an issue.

Macros cannot access the icons. If a macro represents a button, it
should take an optional argument with the icon object. Perhaps we have
to make `icons` in something other than a filter, but this needs to be
worked out.

Perhaps we can make it so that we just write `icons.github` instead of
`icons.get("github")|safe`.
