// The hero mark's markup, read at build time so `HeroLogo` can inline it.
// Vite does not let a script import a file from `public/`, and the file stays
// there because the README points at it too.

import { readFileSync } from 'node:fs'

export default {
  watch: ['../../public/atomic-ferris.svg'],
  load: ([file]) => readFileSync(file, 'utf8')
}
