<script setup>
// An example page: the running example and its code, laid out the way the
// gallery laid them out.
//
// On a wide window the content area is two columns the height of the window.
// The left one has the title, the summary, the example running in an iframe
// and, under it, the page's notes; the right one is the code, with the tabs
// that pick the egui-react or the plain egui version. Each column scrolls on
// its own. A narrow window has room for one pane, so the tabs grow a third
// entry, `example`, and pick which pane is up; the example stays mounted
// under the code, the way the gallery keeps it running behind the code pane.
//
// The wasm behind the iframe is the gallery's `embed` binary: one build for
// the whole site, told which example to draw by the hash
// (`/embed/#counter`, `/embed/#counter/plain`).
//
// The page's markdown comes in through three slots: `summary` (one line under
// the title), `code` (one `[data-version]` wrapper per version, each holding
// an `::: example-source` block) and the default slot (the notes). The code
// is a slot rather than a prop so Shiki highlights it at build time like any
// other fence; this component only toggles a class on the wrapper and the
// stylesheet hides the version that is not selected. That is why the styles
// live in `custom.css`: scoped styles cannot reach slot content the page owns.

import { computed, ref } from 'vue'
import { withBase } from 'vitepress'

const props = defineProps({
  /** The example's directory name, which is also its hash in the embed. */
  name: { type: String, required: true },
  /** Whether this example has a plain egui version to switch to. */
  hasPlain: { type: Boolean, default: false },
  /** Lines of the egui-react version, as the tab label reports them. */
  reactLines: { type: Number, default: 0 },
  /** Lines of the plain egui version. */
  plainLines: { type: Number, default: 0 }
})

/// Which tab is selected: `example`, `react` or `plain`. A wide window shows
/// the example whatever the tab says and has no `example` tab; there
/// `example` and `react` both mean the egui-react code.
const pane = ref('example')

/// The plain version is showing, in the code and in the iframe. An example
/// with no plain version never leaves false, the rule the gallery follows.
const plain = computed(() => props.hasPlain && pane.value === 'plain')

// `site/dev.sh` runs the embed on trunk's own dev server, which rebuilds
// the wasm when the Rust changes; it hands that origin over as
// VITE_EMBED_ORIGIN. A build has no such variable and uses the copy in `dist/`.
const embed = import.meta.env.VITE_EMBED_ORIGIN ?? withBase('/embed/')
const src = computed(() => `${embed}#${props.name}${plain.value ? '/plain' : ''}`)
</script>

<template>
  <div class="example-view" :data-pane="pane">
    <div class="example-main">
      <header class="example-header">
        <h1>{{ name }}</h1>
        <slot name="summary" />
      </header>

      <!-- The narrow window's tabs: which pane is up. The stylesheet hides
           this row on a wide window, where the row in the code column takes
           over. -->
      <div class="example-tabs example-tabs-narrow" role="tablist">
        <button
          type="button"
          :class="{ active: pane === 'example' }"
          :aria-pressed="pane === 'example'"
          @click="pane = 'example'"
        >
          example
        </button>
        <button
          type="button"
          :class="{ active: pane === 'react' }"
          :aria-pressed="pane === 'react'"
          @click="pane = 'react'"
        >
          egui-react · {{ reactLines }} lines
        </button>
        <button
          v-if="hasPlain"
          type="button"
          :class="{ active: pane === 'plain' }"
          :aria-pressed="pane === 'plain'"
          @click="pane = 'plain'"
        >
          plain egui · {{ plainLines }} lines
        </button>
      </div>

      <!-- A fixed box, because the canvas fills whatever it is given and a
           wasm app that resizes the page as it loads would move the text
           under it. `loading="lazy"` keeps a prefetched page from booting
           wasm, and the `key` restarts the app on a switch instead of leaving
           the previous example's state next to the other version's code. -->
      <div class="example-frame">
        <iframe
          :key="src"
          :src="src"
          :title="`running example: ${name}`"
          loading="lazy"
        ></iframe>
      </div>

      <div class="example-notes">
        <slot />
      </div>
    </div>

    <div class="example-side">
      <!-- The wide window's tabs: which version, of the code and the example
           both. Only a row when there is a plain version to pick. -->
      <div v-if="hasPlain" class="example-tabs example-tabs-wide" role="tablist">
        <button
          type="button"
          :class="{ active: !plain }"
          :aria-pressed="!plain"
          @click="pane = 'react'"
        >
          egui-react · {{ reactLines }} lines
        </button>
        <button
          type="button"
          :class="{ active: plain }"
          :aria-pressed="plain"
          @click="pane = 'plain'"
        >
          plain egui · {{ plainLines }} lines
        </button>
      </div>

      <div class="example-code" :class="plain ? 'show-plain' : 'show-react'">
        <slot name="code" />
      </div>
    </div>
  </div>
</template>
