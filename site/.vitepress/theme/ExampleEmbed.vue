<script setup>
// One example, running in an iframe, with its source next to it: the home
// page's counter. The example pages have a layout of their own,
// `ExamplePage.vue`.
//
// The wasm behind the iframe is the gallery's `embed` binary: one build for
// the whole site, told which example to draw by the hash
// (`/embed/#counter`, `/embed/#counter/plain`).
//
// The source is *not* a prop. The page keeps its own code fences and hands
// them to the default slot, so Shiki highlights them at build time; this
// component only toggles a class on their wrapper and the stylesheet hides the
// version that is not selected. That is why the styles live in `custom.css`
// rather than in a `<style scoped>` block here: scoped styles cannot reach
// slot content the page owns.

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

/// Which version is showing. An example with no plain version never leaves
/// false, the same rule the gallery and the embed follow.
const plain = ref(false)

// `site/dev.sh` runs the embed on trunk's own dev server, which rebuilds
// the wasm when the Rust changes; it hands that origin over as
// VITE_EMBED_ORIGIN. A build has no such variable and uses the copy in `dist/`.
const embed = import.meta.env.VITE_EMBED_ORIGIN ?? withBase('/embed/')
const src = computed(() => `${embed}#${props.name}${plain.value ? '/plain' : ''}`)
</script>

<template>
  <div class="example-embed">
    <div v-if="hasPlain" class="example-tabs">
      <button
        type="button"
        :class="{ active: !plain }"
        :aria-pressed="!plain"
        @click="plain = false"
      >
        egui-react · {{ reactLines }} lines
      </button>
      <button
        type="button"
        :class="{ active: plain }"
        :aria-pressed="plain"
        @click="plain = true"
      >
        plain egui · {{ plainLines }} lines
      </button>
    </div>

    <!-- A fixed box, because the canvas fills whatever it is given and a wasm
         app that resizes the page as it loads would move the text under it.
         `loading="lazy"` keeps a prefetched page from booting wasm, and the
         `key` restarts the app on a switch instead of leaving the previous
         example's state next to the other version's code. -->
    <div class="example-frame">
      <iframe
        :key="src"
        :src="src"
        :title="`running example: ${name}`"
        loading="lazy"
      ></iframe>
    </div>

    <div class="example-code" :class="plain ? 'show-plain' : 'show-react'">
      <slot />
    </div>
  </div>
</template>
