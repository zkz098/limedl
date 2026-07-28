import { ref, computed, watch, type Ref } from "vue";

export function usePagination<T>(items: Ref<T[]>, defaultPageSize = 50) {
  const currentPage = ref(1);
  const pageSize = ref<number | null>(defaultPageSize);

  const totalPages = computed(() => {
    if (pageSize.value === null) return 1;
    return Math.max(1, Math.ceil(items.value.length / pageSize.value));
  });

  const paginatedItems = computed(() => {
    if (pageSize.value === null) return items.value;
    const start = (currentPage.value - 1) * pageSize.value;
    return items.value.slice(start, start + pageSize.value);
  });

  const pageStart = computed(() => {
    if (pageSize.value === null) return items.value.length ? 1 : 0;
    return items.value.length ? (currentPage.value - 1) * pageSize.value + 1 : 0;
  });

  const pageEnd = computed(() => {
    if (pageSize.value === null) return items.value.length;
    return items.value.length
      ? Math.min(currentPage.value * pageSize.value, items.value.length)
      : 0;
  });

  function goToPreviousPage() {
    currentPage.value = Math.max(1, currentPage.value - 1);
  }

  function goToNextPage() {
    currentPage.value = Math.min(totalPages.value, currentPage.value + 1);
  }

  // Reset to page 1 when pageSize changes
  watch(pageSize, () => {
    currentPage.value = 1;
  });

  // Clamp currentPage when items shrink
  watch(
    () => items.value.length,
    () => {
      if (currentPage.value > totalPages.value) {
        currentPage.value = totalPages.value;
      }
    },
  );

  return {
    currentPage,
    pageSize,
    totalPages,
    paginatedItems,
    pageStart,
    pageEnd,
    goToPreviousPage,
    goToNextPage,
  };
}
