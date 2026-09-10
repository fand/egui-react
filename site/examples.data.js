// What every example page needs to know about its example, read at build time.
//
// A VitePress data loader, because a markdown page runs in the browser and the
// numbers come from files on disk: the loader runs in Node during the build and
// its result is serialized into the page. `watch` keeps `npm run dev` honest
// while an example is being edited.

import { exampleNames, hasPlain, lineCount } from './lib/example-source.js'

export default {
  watch: ['../examples/*/src/*.rs'],
  load() {
    return Object.fromEntries(
      exampleNames().map((name) => {
        const plain = hasPlain(name)
        return [
          name,
          {
            hasPlain: plain,
            reactLines: lineCount(name),
            plainLines: plain ? lineCount(name, true) : 0
          }
        ]
      })
    )
  }
}
