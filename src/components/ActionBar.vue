<script setup lang="ts">
import { ref } from "vue";
import { useFileDialog } from "../composables/useFileDialog";
import { useDownloader } from "../composables/useDownloader";
import Card from "primevue/card";
import InputText from "primevue/inputtext";
import Button from "primevue/button";
import InputNumber from "primevue/inputnumber";
import Dropdown from "primevue/dropdown";

const fileDialog = useFileDialog();
const { form, submitStart, isStarting } = useDownloader();

const isPickingDirectory = ref(false);

const checksumOptions = [
  { label: "BLAKE3", value: "blake3" },
  { label: "None", value: "none" },
];

async function handlePickDirectory() {
  try {
    isPickingDirectory.value = true;
    await fileDialog.pick();
    if (fileDialog.selectedPath.value) {
      form.destinationDir = fileDialog.selectedPath.value;
    }
  } catch (error) {
    console.error("Failed to pick directory:", error);
  } finally {
    isPickingDirectory.value = false;
  }
}
</script>

<template>
  <Card class="action-bar">
    <template #header>
      <div class="header-content">
        <div>
          <p class="kicker">New transfer</p>
          <h2>Start a download</h2>
        </div>
      </div>
    </template>

    <form @submit.prevent="submitStart" class="form-grid">
      <div class="form-group">
        <label for="url">URL *</label>
        <InputText
          id="url"
          v-model="form.url"
          type="url"
          placeholder="https://example.com/file.zip"
          class="w-full"
          required
        />
      </div>

      <div class="form-group form-full">
        <label for="destdir">Destination Directory *</label>
        <div class="flex gap-2">
          <InputText
            id="destdir"
            v-model="form.destinationDir"
            type="text"
            placeholder="C:\\Downloads"
            class="flex-1"
            readonly
            required
          />
          <Button
            icon="pi pi-folder-open"
            @click="handlePickDirectory"
            :loading="isPickingDirectory"
            severity="secondary"
          />
        </div>
      </div>

      <div class="form-group form-full">
        <label for="filename">File name override</label>
        <InputText
          id="filename"
          v-model="form.fileName"
          type="text"
          placeholder="Optional file name"
          class="w-full"
        />
      </div>

      <div class="form-group">
        <label for="connections">Connections</label>
        <InputNumber
          id="connections"
          v-model.number="form.maxConnections"
          :min="1"
          class="w-full"
        />
      </div>

      <div class="form-group">
        <label for="retries">Retries</label>
        <InputNumber
          id="retries"
          v-model.number="form.maxRetries"
          :min="0"
          class="w-full"
        />
      </div>

      <div class="form-group">
        <label for="checksum">Checksum</label>
        <Dropdown
          id="checksum"
          v-model="form.checksum"
          :options="checksumOptions"
          option-label="label"
          option-value="value"
          class="w-full"
        />
      </div>

      <div class="form-full">
        <Button
          type="submit"
          :loading="isStarting"
          label="Start download"
          class="w-full"
        />
      </div>
    </form>
  </Card>
</template>

<style scoped>
.action-bar {
  width: 100%;
}

.header-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.5rem;
}

.kicker {
  margin: 0;
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--text-color-secondary);
  font-weight: 600;
}

h2 {
  margin: 0.5rem 0 0 0;
  font-size: 1.5rem;
  font-weight: 700;
}

.form-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 1rem;
  padding: 1.5rem;
}

.form-full {
  grid-column: 1 / -1;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.form-group label {
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-color-secondary);
}

.flex {
  display: flex;
}

.gap-2 {
  gap: 0.5rem;
}

.flex-1 {
  flex: 1;
}

.w-full {
  width: 100%;
}

@media (max-width: 640px) {
  .form-grid {
    grid-template-columns: 1fr;
  }

  .form-full {
    grid-column: 1;
  }
}
</style>
