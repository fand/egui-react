<script setup>
// The Examples page: one card per example, with its thumbnail, its name and
// the one-line summary from its `META`.
//
// The thumbnails are `public/thumbs/<name>.png`, drawn by
// `examples/gallery/tests/thumbnails.rs` and committed, since the site build
// has no GPU. An example without one gets its summary where the picture
// would be.

import { withBase } from 'vitepress'
import { data } from '../../examples-index.data.js'
</script>

<template>
  <div class="example-cards">
    <a
      v-for="example in data"
      :key="example.name"
      class="example-card"
      :href="withBase(`/examples/${example.name}.html`)"
    >
      <div class="thumb">
        <img
          v-if="example.thumb"
          :src="withBase(`/thumbs/${example.name}.png`)"
          :alt="`${example.name} running`"
          loading="lazy"
        />
        <span v-else>native only</span>
      </div>
      <h3>{{ example.name }}</h3>
      <p>{{ example.summary }}</p>
    </a>
  </div>
</template>

<style scoped>
.example-cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 16px;
  margin: 24px 0;
}

.example-card {
  display: block;
  border: 1px solid var(--vp-c-bg-soft);
  border-radius: 12px;
  background-color: var(--vp-c-bg-soft);
  overflow: hidden;
  color: inherit;
  text-decoration: none;
  transition: border-color 0.25s;
}

.example-card:hover {
  border-color: var(--vp-c-brand-1);
}

.thumb {
  aspect-ratio: 16 / 10;
  background-color: #101010;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--vp-c-text-3);
  font-size: 13px;
}

.thumb img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: top left;
}

.example-card h3 {
  margin: 12px 16px 4px;
  font-size: 16px;
  font-weight: 600;
  line-height: 24px;
  letter-spacing: -0.02em;
}

.example-card p {
  margin: 0 16px 16px;
  font-size: 14px;
  line-height: 22px;
  color: var(--vp-c-text-2);
}
</style>
