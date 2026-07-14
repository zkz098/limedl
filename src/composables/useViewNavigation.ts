import { computed, ref, type Ref } from "vue";
import { useI18n } from "../i18n";

export interface PersistablePage {
  persistSettings?: () => Promise<boolean>;
}

export interface UseViewNavigationParams {
  settingsPageRef: Ref<PersistablePage | null>;
  labsPageRef: Ref<PersistablePage | null>;
}

function isValidView(view: string): view is "home" | "settings" | "labs" {
  return view === "home" || view === "settings" || view === "labs";
}

export function useViewNavigation(params: UseViewNavigationParams) {
  const { settingsPageRef, labsPageRef } = params;
  const { t } = useI18n();

  const currentView = ref<"home" | "settings" | "labs">("home");
  const pendingView = ref<"home" | "settings" | "labs" | null>(null);
  const settingsHasUnsavedChanges = ref(false);
  const labsHasUnsavedChanges = ref(false);
  const isSavingBeforeNavigation = ref(false);

  const showUnsavedSettingsDialog = computed(() => pendingView.value !== null);
  const pendingViewIsLeavingLabs = computed(
    () => currentView.value === "labs" && pendingView.value !== null,
  );
  const unsavedDialogKicker = computed(() =>
    pendingViewIsLeavingLabs.value ? t("labs.kicker") : t("settings.kicker"),
  );
  const unsavedDialogTitle = computed(() =>
    pendingViewIsLeavingLabs.value ? t("labs.unsavedTitle") : t("dialog.unsavedSettingsTitle"),
  );
  const unsavedDialogMessage = computed(() =>
    pendingViewIsLeavingLabs.value ? t("labs.unsavedMessage") : t("dialog.unsavedSettingsMessage"),
  );

  function navigateTo(view: string) {
    if (!isValidView(view)) {
      return;
    }

    if (view === currentView.value) {
      return;
    }

    const leavingDirtyView =
      (currentView.value === "settings" && settingsHasUnsavedChanges.value) ||
      (currentView.value === "labs" && labsHasUnsavedChanges.value);

    if (leavingDirtyView) {
      pendingView.value = view;
      return;
    }

    currentView.value = view;
  }

  function cancelDiscardSettings() {
    pendingView.value = null;
  }

  function confirmDiscardSettings() {
    const nextView = pendingView.value;
    pendingView.value = null;
    settingsHasUnsavedChanges.value = false;
    labsHasUnsavedChanges.value = false;

    if (nextView) {
      currentView.value = nextView;
    }
  }

  async function saveSettingsAndNavigate() {
    if (isSavingBeforeNavigation.value) {
      return;
    }

    isSavingBeforeNavigation.value = true;
    try {
      let saved = false;
      if (currentView.value === "settings") {
        saved = (await settingsPageRef.value?.persistSettings?.()) ?? false;
      } else if (currentView.value === "labs") {
        saved = (await labsPageRef.value?.persistSettings?.()) ?? false;
      }

      if (!saved) {
        return;
      }

      const nextView = pendingView.value;
      pendingView.value = null;
      settingsHasUnsavedChanges.value = false;
      labsHasUnsavedChanges.value = false;
      if (nextView) {
        currentView.value = nextView;
      }
    } finally {
      isSavingBeforeNavigation.value = false;
    }
  }

  return {
    currentView,
    settingsHasUnsavedChanges,
    labsHasUnsavedChanges,
    isSavingBeforeNavigation,
    navigateTo,
    cancelDiscardSettings,
    confirmDiscardSettings,
    saveSettingsAndNavigate,
    showUnsavedSettingsDialog,
    unsavedDialogKicker,
    unsavedDialogTitle,
    unsavedDialogMessage,
  };
}
