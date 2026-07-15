import { ref } from "vue";

export function useAsyncGuard() {
  const isBusy = ref(false);

  async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
    if (isBusy.value) {
      return undefined;
    }
    isBusy.value = true;
    let result: T | undefined;
    let error: unknown;
    try {
      result = await fn();
    } catch (e) {
      error = e;
    }
    isBusy.value = false;
    if (error) throw error;
    return result;
  }

  return { isBusy, run };
}
