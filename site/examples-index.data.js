// The cards on the Examples page: every example, in the gallery's order, with
// its one-line summary and whether it has a thumbnail.
//
// A data loader, like `examples.data.js`, because the summaries live in the
// example crates' `META` blocks on disk and the page runs in the browser.

import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { summary } from './lib/example-source.js'
import { EXAMPLE_ORDER } from './lib/examples-order.js'

const SITE = path.dirname(fileURLToPath(import.meta.url))

export default {
  watch: ['../examples/*/src/lib.rs', './public/thumbs/*.png'],
  load() {
    return EXAMPLE_ORDER.map((name) => ({
      name,
      summary: summary(name),
      // `shell` has none: it does not run inside the gallery's `Running`, so
      // `tests/thumbnails.rs` cannot draw it.
      thumb: fs.existsSync(path.join(SITE, 'public', 'thumbs', `${name}.png`))
    }))
  }
}
