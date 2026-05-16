<template>
  <section class="ui-card">
    <header v-if="$slots.header" class="ui-card__header">
      <slot name="header" />
    </header>
    <div class="ui-card__body">
      <slot />
    </div>
    <footer v-if="$slots.footer" class="ui-card__footer">
      <slot name="footer" />
    </footer>
  </section>
</template>

<style scoped>
.ui-card {
  background: color-mix(in srgb, var(--color-panel) var(--surface-card-alpha), transparent);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-card);
  backdrop-filter: blur(var(--surface-blur));
  overflow: hidden;
  transition:
    transform 0.25s ease,
    border-color 0.25s ease,
    box-shadow 0.25s ease;
}

.ui-card:hover {
  transform: translateY(-0.125rem);
  border-color: var(--color-accent-soft-border);
  box-shadow: var(--shadow-card-hover);
}

.ui-card__header,
.ui-card__footer {
  padding: 1.125rem 1.25rem;
}

.ui-card__body {
  padding: 1.125rem 1.25rem 1.25rem;
}

.ui-card__header + .ui-card__body {
  padding-top: 0;
}

.ui-card__footer {
  border-top: 1px solid var(--color-border);
}
</style>
