<script setup lang="ts">
import { computed, inject } from "vue";
import { FIELD_ASSOCIATION } from "./field-association";

const props = withDefaults(
  defineProps<{
    modelValue: number;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
    id?: string;
    ariaLabel?: string;
    ariaLabelledby?: string;
  }>(),
  {
    min: 0,
    max: 100,
    step: 1,
    disabled: false,
    id: undefined,
    ariaLabel: undefined,
    ariaLabelledby: undefined,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: number];
}>();

const fieldAssociation = inject(FIELD_ASSOCIATION, null);
const resolvedId = computed(() => props.id ?? fieldAssociation?.id ?? undefined);

const fillPercent = computed(() => {
  const range = props.max - props.min;
  if (range <= 0) return 0;
  const clamped = Math.min(Math.max(props.modelValue, props.min), props.max);
  return ((clamped - props.min) / range) * 100;
});

function onInput(event: Event) {
  const target = event.target as HTMLInputElement;
  emit("update:modelValue", Number(target.value));
}
</script>

<template>
  <div class="ui-slider" :class="{ 'ui-slider--disabled': disabled }">
    <input
      :id="resolvedId"
      class="ui-slider__input"
      type="range"
      :value="modelValue"
      :min="min"
      :max="max"
      :step="step"
      :disabled="disabled"
      :aria-label="ariaLabel"
      :aria-labelledby="ariaLabelledby"
      :style="{ '--fill': `${fillPercent}%` } as unknown as Record<string, string>"
      @input="onInput"
    />
  </div>
</template>

<style scoped>
.ui-slider {
  width: 100%;
  display: flex;
  align-items: center;
  min-height: 1.5rem;
}

.ui-slider--disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.ui-slider__input {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 1.5rem;
  margin: 0;
  padding: 0;
  background: transparent;
  cursor: pointer;
}

.ui-slider__input:disabled {
  cursor: not-allowed;
}

/* Track — WebKit */
.ui-slider__input::-webkit-slider-runnable-track {
  height: 0.4rem;
  border-radius: var(--radius-pill);
  background: linear-gradient(
    to right,
    var(--color-accent-strong) 0%,
    var(--color-accent-strong) var(--fill),
    var(--color-progress-track) var(--fill),
    var(--color-progress-track) 100%
  );
  transition: background 0.15s ease;
}

.ui-slider__input:disabled::-webkit-slider-runnable-track {
  background: var(--color-progress-track);
}

/* Track — Firefox */
.ui-slider__input::-moz-range-track {
  height: 0.4rem;
  border: none;
  border-radius: var(--radius-pill);
  background: var(--color-progress-track);
}

.ui-slider__input::-moz-range-progress {
  height: 0.4rem;
  border-radius: var(--radius-pill);
  background: var(--color-accent-strong);
}

.ui-slider__input:disabled::-moz-range-progress {
  background: var(--color-text-soft);
}

/* Thumb — WebKit */
.ui-slider__input::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 1.05rem;
  height: 1.05rem;
  margin-top: -0.325rem;
  border-radius: var(--radius-pill);
  border: 2px solid var(--color-panel);
  background: var(--color-accent-strong);
  box-shadow: var(--shadow-soft);
  cursor: pointer;
  transition:
    transform 0.15s ease,
    box-shadow 0.15s ease,
    background-color 0.15s ease;
}

.ui-slider__input:active::-webkit-slider-thumb {
  transform: scale(1.08);
}

.ui-slider__input:disabled::-webkit-slider-thumb {
  background: var(--color-text-soft);
  border-color: var(--color-panel);
  box-shadow: none;
  cursor: not-allowed;
}

/* Thumb — Firefox */
.ui-slider__input::-moz-range-thumb {
  width: 1.05rem;
  height: 1.05rem;
  border-radius: var(--radius-pill);
  border: 2px solid var(--color-panel);
  background: var(--color-accent-strong);
  box-shadow: var(--shadow-soft);
  cursor: pointer;
  transition:
    transform 0.15s ease,
    box-shadow 0.15s ease;
}

.ui-slider__input:active::-moz-range-thumb {
  transform: scale(1.08);
}

.ui-slider__input:disabled::-moz-range-thumb {
  background: var(--color-text-soft);
  cursor: not-allowed;
}

/* Focus */
.ui-slider__input:focus-visible {
  outline: none;
}

.ui-slider__input:focus-visible::-webkit-slider-thumb {
  box-shadow:
    0 0 0 2px var(--color-panel),
    0 0 0 4px var(--color-focus-ring);
}

.ui-slider__input:focus-visible::-moz-range-thumb {
  box-shadow:
    0 0 0 2px var(--color-panel),
    0 0 0 4px var(--color-focus-ring);
}

/* Hover — slightly brighter track */
.ui-slider__input:not(:disabled):hover::-webkit-slider-runnable-track {
  filter: brightness(1.02);
}
</style>
