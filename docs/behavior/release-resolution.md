# Release resolution behavior

This document specifies LumilioCL behavior in domain terms. The vendored
implementation was consulted only through mapping IDs recorded in
`docs/architecture.md`; its package layout and object model are not the design
of this subsystem.

## Dependency coordinates

A dependency coordinate contains a namespace, component name, release string,
optional variant, and optional file extension. The accepted textual forms are
`namespace:component:release`, `namespace:component:release:variant`, and either
form with `@extension` appended to the final segment. The default extension is
`jar`.

Resolution produces a stable repository-relative path:

1. Split the namespace on dots and use each segment as a directory.
2. Append component and release directories.
3. Build the file name from component, release, optional variant, and extension.

Malformed segment counts, empty required segments, empty explicit extensions,
and multiple extension separators are errors.

## Environment rules

An environment contains an operating-system family, architecture family,
reported OS version, and named boolean capabilities.

- An absent or empty rule list allows the item.
- A non-empty rule list begins denied.
- Rules are visited in declaration order. A rule whose selectors all match
  replaces the current decision; therefore the last matching rule wins.
- OS and architecture selectors are optional. Known OS names must match the
  host family. A Linux selector also accepts BSD-family hosts.
- A combined selector such as `windows-x86` matches both dimensions.
- Optional version and architecture patterns must match the entire reported
  value. An invalid pattern does not match.
- Every named capability in a rule must be present with the exact boolean
  value. Missing capabilities do not match.

## Argument templates

Version JSON accepts argument entries as either a string or an object containing
rules plus `value`/`values`. A value may be one string or a list of strings.
An applicable entry emits all non-null values in order. Variables use the
`${name}` form; known variables expand and unknown variables remain unchanged so
the caller can diagnose an incomplete launch context.

Legacy game argument strings and modern structured arguments are distinct input
forms. If legacy arguments are present, they are tokenized and used as the game
arguments. Structured JVM arguments remain available, with launcher defaults
used only when the manifest provides no JVM group.

## Libraries and native variants

Libraries retain source order. A library excluded by environment rules emits no
classpath or native entry. A library with platform-native metadata resolves one
native variant using the host OS and architecture; `${arch}` expands to the
architecture bit width. Explicit artifact download paths and URLs take priority,
then the coordinate-derived path and configured repository base are used.

Classpath dependencies and native archives are separate outputs. Duplicate
classpath files are collapsed deterministically without sorting away declaration
order. Native archives include their extraction exclusion rules.

## Manifest decoding and inheritance

The protocol decoder requires a non-empty release id and preserves declaration
order for libraries and download maps. Unknown JSON members are retained at the
boundary so editing known fields does not destroy third-party metadata.

When a child release inherits from a parent:

- scalar launch fields use the child value when present;
- structured parent arguments precede child arguments;
- child libraries precede inherited parent libraries;
- environment rules retain parent-before-child declaration order;
- the required launcher revision becomes the maximum of both values;
- the result has no unresolved parent link.

Resolution rejects inheritance cycles and missing parents. Launch assembly
rejects any manifest that still carries unresolved inheritance or pending patch
operations.

## Launch specification

Launch resolution is pure: it receives a resolved release, host environment,
filesystem layout, identity/session values, launcher identity, optional feature
flags, and optional resolution. It returns:

- selected Java requirement and main class;
- JVM arguments and game arguments after rule evaluation and expansion;
- an ordered classpath;
- native archives and their extraction policy;
- the client artifact, asset index, and logging configuration required by the
  installation and process layers.

Missing required fields or variables are explicit errors. No filesystem access,
network transfer, process spawning, or GPUI types belong in this subsystem.
