// The cards on the Examples page: every example, in the gallery's order, with
// its one-line summary, in both languages, and whether it has a thumbnail.
//
// A data loader, like `examples.data.js`, because the summaries live in the
// example crates' `META` blocks on disk and the page runs in the browser.

import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { summary } from './lib/example-source.js'
import { summaryJa } from './lib/summaries-ja.js'
import { EXAMPLE_ORDER } from './lib/examples-order.js'

const SITE = path.dirname(fileURLToPath(import.meta.url))

export default {
  watch: ['../examples/*/src/lib.rs', './lib/summaries-ja.js', './public/thumbs/*.png'],
  load() {
    return EXAMPLE_ORDER.map((name) => ({
      name,
      summary: summary(name),
      summaryJa: summaryJa(name),
      // Missing only until `tests/thumbnails.rs` has been run for a new
      // example.
      thumb: fs.existsSync(path.join(SITE, 'public', 'thumbs', `${name}.png`))
    }))
  }
}
