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
  background: var(--color-panel);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-card);
  overflow: hidden;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.ui-card:hover {
  border-color: var(--color-border-strong);
  box-shadow: var(--shadow-card-hover);
}

.ui-card__header,
.ui-card__footer {
  padding: 1rem 1.25rem;
}

.ui-card__body {
  padding: 1rem 1.25rem 1.25rem;
}

.ui-card__header + .ui-card__body {
  padding-top: 0;
}

.ui-card__footer {
  border-top: 1px solid var(--color-border);
}
</style>
