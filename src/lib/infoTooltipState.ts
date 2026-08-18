import { ref } from "vue";

// Module-level singleton: only one InfoTooltip may be open at a time.
// Lives in its own module so every InfoTooltip instance shares the same
// state while the component only imports `vue` once (avoids S3863 duplicate
// imports that would arise from a separate module-scope `<script>` block).
export const activeTrigger = ref<HTMLElement | null>(null);
