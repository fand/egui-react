// The example sources, as the pages show them.
//
// A port of `gallery::shown_source` (examples/gallery/src/lib.rs): the pages
// and the gallery must show the same thing, and both read the example crates
// themselves, so neither can drift from the code that actually runs. Kept as a
// port rather than shelling out to cargo because the site build has no Rust
// toolchain of its own in the fast CI path.
//
// The markdown side is a block rule rather than an HTML-emitting container:
// `::: example-source counter` becomes a `fence` token, so Shiki highlights it
// at build time exactly like a hand-written ```rust block.

import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

/// The repository root, two levels above this file (site/lib).
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..')

/// The line every example's `META` block opens with. Both a marker of "this is
/// an example" and the start of what the pages drop.
const META_START = 'pub const META: Meta = Meta {'

/** Where an example's source lives: `lib.rs`, or `plain.rs` for the plain egui version. */
export function sourcePath(name, plain = false) {
  return path.join(ROOT, 'examples', name, 'src', plain ? 'plain.rs' : 'lib.rs')
}

/**
 * Every example crate, by directory name.
 *
 * An example is a directory under `examples/` whose `src/lib.rs` declares a
 * `META`: that is what makes it an example rather than the gallery, the `Meta`
 * crate itself or some other member of the workspace.
 */
export function exampleNames() {
  return fs
    .readdirSync(path.join(ROOT, 'examples'), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .filter((name) => {
      const file = sourcePath(name)
      // At the start of a line: the gallery *mentions* the marker inside its
      // own copy of this filter, indented, and is not an example.
      return (
        fs.existsSync(file) &&
        fs
          .readFileSync(file, 'utf8')
          .split('\n')
          .some((line) => line.startsWith(META_START))
      )
    })
    .sort()
}

/**
 * The one-line summary from the example's `META`, as the gallery and the
 * README show it. Throws when there is none, so a card never comes out blank.
 */
export function summary(name) {
  const source = fs.readFileSync(sourcePath(name), 'utf8')
  // `summary: "..."` up to the closing quote. A `\` at a line end continues the
  // literal on the next line, with its leading spaces dropped, as in Rust.
  const match = /^\s*summary:\s*"((?:[^"\\]|\\[\s\S])*)"/m.exec(source)
  if (!match) {
    throw new Error(`example-source: no META summary in ${path.relative(ROOT, sourcePath(name))}`)
  }
  return JSON.parse(`"${match[1].replace(/\\\n\s*/g, '')}"`)
}

/** Whether this example has a plain egui version next to it. */
export function hasPlain(name) {
  return fs.existsSync(sourcePath(name, true))
}

/**
 * The source of one example as the pages show it.
 *
 * Throws when the file is missing, so a page naming an example that does not
 * exist fails the build instead of rendering an empty block.
 */
export function shownSource(name, plain = false) {
  const file = sourcePath(name, plain)
  if (!fs.existsSync(file)) {
    throw new Error(`example-source: no ${path.relative(ROOT, file)} for "${name}"`)
  }
  return shown(fs.readFileSync(file, 'utf8'))
}

/** How many lines [`shownSource`] returns: the tab labels are these numbers. */
export function lineCount(name, plain = false) {
  return shownSource(name, plain).trimEnd().split('\n').length
}

/**
 * The example, without the gallery's own plumbing.
 *
 * Dropped: the crate doc comment at the top (`//!` — design notes, for the
 * repository, not for the page next to the running example), the `META` block
 * and its import, and anything between a `// gallery:hide` line and the next
 * `// gallery:show`. Runs of blank lines that leaves behind are folded into
 * one. The arms below are in the same order as the Rust `match` they came
 * from, because the order is what makes a `};` inside a hidden region behave.
 */
export function shown(source) {
  const lines = source.split('\n').map((line) => line.replace(/\r$/, ''))
  // `str::lines` yields nothing for the newline that ends the last line.
  if (lines.length > 0 && lines[lines.length - 1] === '') {
    lines.pop()
  }
  let start = 0
  while (start < lines.length && lines[start].startsWith('//!')) {
    start += 1
  }
  let out = ''
  let hidden = false
  let inMeta = false
  let blank = true
  for (const line of lines.slice(start)) {
    const trimmed = line.trim()
    if (trimmed === '// gallery:hide') {
      hidden = true
    } else if (trimmed === '// gallery:show') {
      hidden = false
    } else if (hidden) {
      // Nothing: the region is the standalone binary's plumbing.
    } else if (trimmed === 'use example_meta::Meta;' || trimmed === 'pub mod plain;') {
      // Nothing: the import exists only for the block below, and the plain
      // version is the gallery's, not the example's.
    } else if (inMeta) {
      inMeta = line !== '};'
    } else if (line.startsWith(META_START)) {
      inMeta = true
    } else if (trimmed === '' && blank) {
      // Nothing: a run of blank lines is folded into the first of them.
    } else {
      blank = line === ''
      out += line + '\n'
    }
  }
  return out.trimEnd() + '\n'
}

/**
 * The markdown-it rule behind `::: example-source <name> [plain]`.
 *
 * The block spans two lines, the opener and a closing `:::`, so a page reads
 * like any other VitePress container.
 */
export function exampleSourcePlugin(md) {
  const opener = /^:::\s+example-source\s+([a-z0-9-]+)(?:\s+(plain))?\s*$/

  md.block.ruler.before(
    'fence',
    'example_source',
    (state, startLine, endLine, silent) => {
      const text = (line) =>
        state.src.slice(state.bMarks[line] + state.tShift[line], state.eMarks[line]).trim()
      const match = opener.exec(text(startLine))
      if (!match) {
        return false
      }
      if (silent) {
        return true
      }
      let line = startLine + 1
      while (line < endLine && text(line) !== ':::') {
        line += 1
      }
      if (line >= endLine || text(line) !== ':::') {
        throw new Error(`example-source: "${match[0]}" is not closed by :::`)
      }
      // A fence token, not raw HTML: the highlighting is then VitePress's own,
      // done at build time, and the block gets the copy button and the theme
      // every other code block on the page has.
      const plain = match[2] === 'plain'
      // VitePress watches whatever a page's `env.includes` names (its own
      // `<<<` snippets go through the same list), so an edit to the example
      // re-renders the page under `vitepress dev`.
      state.env.includes?.push(sourcePath(match[1], plain))
      const token = state.push('fence', 'code', 0)
      token.info = 'rust'
      token.content = shownSource(match[1], plain)
      token.markup = '```'
      token.map = [startLine, line + 1]
      state.line = line + 1
      return true
    },
    { alt: [] }
  )
}
